//! Git-backed history for the data root. The whole root (all languages) is one
//! repo; this is the only module that touches `gix`.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

const IGNORED_FILES: &[&str] = &["clock.json"];

pub struct CommitInfo {
    pub id: String,
    pub message: String,
}

fn open_or_init(root: &Path) -> Result<gix::Repository> {
    std::fs::create_dir_all(root)?;
    if gix::open(root).is_err() {
        gix::init(root).context("git init of the data root failed")?;
        write_gitignore(root)?;
    }
    // Force hibi's identity via config overrides: the reflog committer comes
    // from config (not the signature passed to `commit_as`), so without this,
    // committing fails for anyone with no global git identity set.
    let opts = gix::open::Options::isolated()
        .config_overrides(["user.name=hibi", "user.email=hibi@localhost"]);
    gix::open_opts(root, opts).context("opening the data root repo failed")
}

fn write_gitignore(root: &Path) -> Result<()> {
    let path = root.join(".gitignore");
    if !path.exists() {
        let body: String = IGNORED_FILES.iter().map(|f| format!("{f}\n")).collect();
        std::fs::write(path, body)?;
    }
    Ok(())
}

/// A fixed identity, so committing doesn't depend on the user's git config.
fn signature() -> gix::actor::Signature {
    let now = chrono::Local::now();
    gix::actor::Signature {
        name: "hibi".into(),
        email: "hibi@localhost".into(),
        time: gix::date::Time::new(now.timestamp(), now.offset().local_minus_utc()),
    }
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files(root, &path, out)?;
        } else {
            let rel = path.strip_prefix(root)?.to_path_buf();
            let rel_str = rel.to_string_lossy();
            if IGNORED_FILES.iter().any(|f| rel_str == **f) {
                continue;
            }
            out.push(rel);
        }
    }
    Ok(())
}

/// Commit the current worktree. `description` is prefixed with a local
/// timestamp. Returns `false` when nothing changed since the last commit.
pub fn commit_all(root: &Path, description: &str) -> Result<bool> {
    let repo = open_or_init(root)?;

    // The tree is rebuilt from scratch each time, so deletions are captured too.
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;

    let empty = gix::ObjectId::empty_tree(repo.object_hash());
    let mut editor = repo.edit_tree(empty)?;
    for rel in &files {
        let bytes = std::fs::read(root.join(rel))?;
        let blob = repo.write_blob(bytes)?.detach();
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        editor.upsert(rel_str.as_str(), gix::object::tree::EntryKind::Blob, blob)?;
    }
    let tree_id = editor.write()?.detach();

    let parent = repo.head_id().ok().map(|id| id.detach());
    if let Some(parent_id) = parent {
        let parent_tree = repo.find_commit(parent_id)?.tree_id()?.detach();
        if parent_tree == tree_id {
            return Ok(false);
        }
    }

    let message = format!(
        "{} · {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M"),
        description
    );
    let sig = signature();
    let parents: Vec<gix::ObjectId> = parent.into_iter().collect();
    repo.commit_as(sig.to_ref(), sig.to_ref(), "HEAD", message, tree_id, parents)
        .context("git commit failed")?;
    Ok(true)
}

/// Commit history, newest first.
pub fn log(root: &Path) -> Result<Vec<CommitInfo>> {
    let repo = open_or_init(root)?;
    let Ok(head) = repo.head_id() else {
        return Ok(Vec::new()); // no commits yet
    };

    let mut out = Vec::new();
    for info in repo.rev_walk(Some(head.detach())).all()? {
        let info = info?;
        let commit = info.object()?;
        let message = commit.message_raw_sloppy().to_string();
        out.push(CommitInfo {
            id: info.id.to_hex().to_string(),
            message: message.lines().next().unwrap_or("").to_string(),
        });
    }
    Ok(out)
}

/// Contents of `rel_path` at `commit_hex`, or `None` if it didn't exist then.
pub fn file_at_commit(root: &Path, commit_hex: &str, rel_path: &str) -> Result<Option<Vec<u8>>> {
    let repo = open_or_init(root)?;
    let id = repo.rev_parse_single(commit_hex)?;
    let commit = id.object()?.try_into_commit()?;
    let tree = commit.tree()?;
    match tree.lookup_entry_by_path(Path::new(rel_path))? {
        Some(entry) => Ok(Some(entry.object()?.data.clone())),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hibi-git-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn commit_then_read_back_round_trips() {
        let root = temp_root("roundtrip");
        write(&root, "japanese/hibi.json", "{\"v\":1}");

        assert!(commit_all(&root, "japanese: first").unwrap());

        let history = log(&root).unwrap();
        assert_eq!(history.len(), 1);
        assert!(history[0].message.ends_with("japanese: first"));

        let bytes = file_at_commit(&root, &history[0].id, "japanese/hibi.json")
            .unwrap()
            .unwrap();
        assert_eq!(String::from_utf8(bytes).unwrap(), "{\"v\":1}");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unchanged_worktree_makes_no_commit() {
        let root = temp_root("nochange");
        write(&root, "japanese/hibi.json", "{}");
        assert!(commit_all(&root, "first").unwrap());
        assert!(!commit_all(&root, "second").unwrap());
        assert_eq!(log(&root).unwrap().len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn log_is_newest_first() {
        let root = temp_root("order");
        write(&root, "japanese/hibi.json", "{\"v\":1}");
        commit_all(&root, "one").unwrap();
        write(&root, "japanese/hibi.json", "{\"v\":2}");
        commit_all(&root, "two").unwrap();

        let history = log(&root).unwrap();
        assert_eq!(history.len(), 2);
        assert!(history[0].message.ends_with("two"));
        assert!(history[1].message.ends_with("one"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn clock_json_is_never_committed() {
        let root = temp_root("ignore");
        write(&root, "japanese/hibi.json", "{}");
        write(&root, "clock.json", "running");

        assert!(commit_all(&root, "clock present").unwrap());

        let id = log(&root).unwrap()[0].id.clone();
        assert!(file_at_commit(&root, &id, "clock.json").unwrap().is_none());
        assert!(file_at_commit(&root, &id, "japanese/hibi.json").unwrap().is_some());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn gitignore_written_on_init() {
        let root = temp_root("gitignore");
        write(&root, "japanese/hibi.json", "{}");
        commit_all(&root, "first").unwrap();
        let ignore = fs::read_to_string(root.join(".gitignore")).unwrap();
        assert!(ignore.contains("clock.json"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn log_on_fresh_repo_is_empty() {
        let root = temp_root("fresh");
        assert!(log(&root).unwrap().is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_file_at_commit_is_none() {
        let root = temp_root("missing");
        write(&root, "japanese/hibi.json", "{}");
        commit_all(&root, "first").unwrap();
        let id = log(&root).unwrap()[0].id.clone();
        assert!(file_at_commit(&root, &id, "spanish/hibi.json").unwrap().is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn restore_reinstates_old_contents_as_new_commit() {
        let root = temp_root("restore");
        write(&root, "japanese/hibi.json", "{\"v\":1}");
        commit_all(&root, "v1").unwrap();
        write(&root, "japanese/hibi.json", "{\"v\":2}");
        commit_all(&root, "v2").unwrap();

        let history = log(&root).unwrap();
        let v1_id = history.iter().find(|c| c.message.ends_with("v1")).unwrap().id.clone();
        let old = file_at_commit(&root, &v1_id, "japanese/hibi.json").unwrap().unwrap();

        fs::write(root.join("japanese/hibi.json"), old).unwrap();
        assert!(commit_all(&root, "restore to v1").unwrap());

        assert_eq!(
            fs::read_to_string(root.join("japanese/hibi.json")).unwrap(),
            "{\"v\":1}"
        );
        assert_eq!(log(&root).unwrap().len(), 3);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn signature_uses_hibi_identity() {
        let sig = signature();
        assert_eq!(sig.name.to_string(), "hibi");
        assert_eq!(sig.email.to_string(), "hibi@localhost");
    }
}
