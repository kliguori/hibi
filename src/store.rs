use anyhow::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Type {
    pub id: i64,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Mode {
    pub id: i64,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Source {
    pub id: i64,
    pub name: String,
    pub type_id: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Session {
    pub id: i64,
    pub source_id: i64,
    pub mode_id: i64,
    pub minutes: i64,
    pub date: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Database {
    pub types: Vec<Type>,
    pub modes: Vec<Mode>,
    pub sources: Vec<Source>,
    pub sessions: Vec<Session>,
    pub next_id: i64,
}

fn project_dirs() -> ProjectDirs {
    ProjectDirs::from("", "", "hibi").expect("could not determine a home directory")
}

pub fn data_root() -> Result<PathBuf> {
    Ok(project_dirs().data_dir().to_path_buf())
}

pub fn config_file() -> Result<PathBuf> {
    Ok(project_dirs().config_dir().join("config.json"))
}

pub fn language_dir(root: &Path, lang: &str) -> PathBuf {
    root.join(lang)
}

pub fn db_file(root: &Path, lang: &str) -> PathBuf {
    language_dir(root, lang).join("hibi.json")
}

pub fn backups_dir(root: &Path, lang: &str) -> PathBuf {
    language_dir(root, lang).join("backups")
}

pub fn load(path: &Path) -> Result<Database> {
    if !path.exists() {
        return Ok(Database::default());
    }
    let text = fs::read_to_string(path)?;
    let db: Database = serde_json::from_str(&text)?;
    Ok(db)
}

pub fn save(path: &Path, db: &Database) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(db)?;
    fs::write(path, text)?;
    Ok(())
}

/// Compact serialization, for detecting whether a command changed anything.
pub fn snapshot(db: &Database) -> Result<String> {
    Ok(serde_json::to_string(db)?)
}

/// Save the dataset and take a pruned backup.
pub fn commit(root: &Path, lang: &str, db: &Database, timestamp: &str, keep: usize) -> Result<()> {
    let db_path = db_file(root, lang);
    save(&db_path, db)?;
    create_backup(&db_path, &backups_dir(root, lang), timestamp, keep)?;
    Ok(())
}

pub const SAMPLE_LANG: &str = "sample";

/// Demo data. Dates are anchored to 2026-07-07 so the rolling windows — and the
/// tests — have known values.
pub fn sample_database() -> Database {
    Database {
        types: vec![
            Type {
                id: 1,
                name: "youtube".to_string(),
            },
            Type {
                id: 2,
                name: "podcast".to_string(),
            },
        ],
        modes: vec![
            Mode {
                id: 3,
                name: "watching".to_string(),
            },
            Mode {
                id: 4,
                name: "listening".to_string(),
            },
        ],
        sources: vec![
            Source {
                id: 5,
                name: "Show A".to_string(),
                type_id: 1,
            },
            Source {
                id: 6,
                name: "Pod B".to_string(),
                type_id: 2,
            },
        ],
        sessions: vec![
            Session {
                id: 10,
                source_id: 5,
                mode_id: 3,
                minutes: 30,
                date: "2026-07-07".to_string(),
            },
            Session {
                id: 11,
                source_id: 6,
                mode_id: 4,
                minutes: 20,
                date: "2026-07-06".to_string(),
            },
            Session {
                id: 12,
                source_id: 5,
                mode_id: 3,
                minutes: 10,
                date: "2026-07-05".to_string(),
            },
            Session {
                id: 13,
                source_id: 6,
                mode_id: 4,
                minutes: 40,
                date: "2026-06-20".to_string(),
            },
            Session {
                id: 14,
                source_id: 5,
                mode_id: 3,
                minutes: 60,
                date: "2026-05-01".to_string(),
            },
            Session {
                id: 15,
                source_id: 6,
                mode_id: 4,
                minutes: 50,
                date: "2026-01-01".to_string(),
            },
            Session {
                id: 16,
                source_id: 5,
                mode_id: 3,
                minutes: 100,
                date: "2024-06-01".to_string(),
            },
        ],
        next_id: 100,
    }
}

/// Seed the read-only sample dataset on first run; returns true if it did.
pub fn ensure_initialized() -> Result<bool> {
    let root = data_root()?;
    if root.exists() {
        return Ok(false);
    }
    save(&db_file(&root, SAMPLE_LANG), &sample_database())?;
    Ok(true)
}

pub fn backup_filename(timestamp: &str) -> String {
    format!("hibi-{}.json", timestamp)
}

/// Backups to delete, oldest first. Timestamped names sort chronologically, so
/// this is just sort-and-drop-the-front.
pub fn backups_to_prune(existing: &[String], keep: usize) -> Vec<String> {
    let mut names = existing.to_vec();
    names.sort();
    if names.len() <= keep {
        return Vec::new();
    }
    let remove = names.len() - keep;
    names.into_iter().take(remove).collect()
}

pub fn create_backup(
    db_path: &Path,
    backups_dir: &Path,
    timestamp: &str,
    keep: usize,
) -> Result<()> {
    if !db_path.exists() {
        return Ok(());
    }
    fs::create_dir_all(backups_dir)?;
    let dest = backups_dir.join(backup_filename(timestamp));
    fs::copy(db_path, &dest)?;

    for name in backups_to_prune(&list_backups(backups_dir)?, keep) {
        let _ = fs::remove_file(backups_dir.join(name));
    }
    Ok(())
}

/// Restore from a backup, snapshotting the current state first so a wrong
/// restore can itself be undone.
pub fn restore_backup(
    db_path: &Path,
    backups_dir: &Path,
    backup_name: &str,
    safety_timestamp: &str,
    keep: usize,
) -> Result<()> {
    // Read it before the safety snapshot runs — the prune could delete it if
    // it's the oldest and we're at the keep limit.
    let contents = fs::read(backups_dir.join(backup_name))?;

    if db_path.exists() {
        create_backup(db_path, backups_dir, safety_timestamp, keep)?;
    }

    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(db_path, contents)?;
    Ok(())
}

pub fn list_backups(dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    if !dir.exists() {
        return Ok(names);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if let Some(name) = entry.file_name().to_str() {
            if name.starts_with("hibi-") && name.ends_with(".json") {
                names.push(name.to_string());
            }
        }
    }
    Ok(names)
}

/// The single global timer — a stopwatch. `accumulated_seconds` is finished
/// segments; `running_since` (RFC3339) is the current segment's start, or None
/// while paused.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Clock {
    pub language: String,
    pub source_id: i64,
    pub mode_id: i64,
    pub started_at: String,
    pub accumulated_seconds: i64,
    pub running_since: Option<String>,
}

pub fn clock_file(root: &Path) -> PathBuf {
    root.join("clock.json")
}

pub fn load_clock(path: &Path) -> Result<Option<Clock>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    let clock: Clock = serde_json::from_str(&text)?;
    Ok(Some(clock))
}

pub fn save_clock(path: &Path, clock: &Clock) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(clock)?)?;
    Ok(())
}

pub fn clear_clock(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// The running segment is clamped to ≥0 so a backwards clock jump can't
/// subtract time.
pub fn clock_total_seconds(
    accumulated_seconds: i64,
    running_since_unix: Option<i64>,
    now_unix: i64,
) -> i64 {
    match running_since_unix {
        Some(since) => accumulated_seconds + (now_unix - since).max(0),
        None => accumulated_seconds,
    }
}

pub fn seconds_to_minutes_floor(seconds: i64) -> i64 {
    seconds / 60
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_helpers_build_expected_layout() {
        let root = Path::new("/data");
        assert_eq!(language_dir(root, "japanese"), Path::new("/data/japanese"));
        assert_eq!(
            db_file(root, "japanese"),
            Path::new("/data/japanese/hibi.json")
        );
        assert_eq!(
            backups_dir(root, "japanese"),
            Path::new("/data/japanese/backups")
        );
    }

    #[test]
    fn backup_filename_wraps_timestamp() {
        assert_eq!(
            backup_filename("2026-07-07T09-30-00"),
            "hibi-2026-07-07T09-30-00.json"
        );
    }

    #[test]
    fn prune_removes_oldest_beyond_keep() {
        let existing = vec![
            "hibi-2026-03-01T00-00-00.json".to_string(),
            "hibi-2026-01-01T00-00-00.json".to_string(),
            "hibi-2026-02-01T00-00-00.json".to_string(),
        ];
        assert_eq!(
            backups_to_prune(&existing, 2),
            vec!["hibi-2026-01-01T00-00-00.json".to_string()]
        );
    }

    #[test]
    fn prune_is_noop_within_limit() {
        let existing = vec!["hibi-a.json".to_string(), "hibi-b.json".to_string()];
        assert!(backups_to_prune(&existing, 5).is_empty());
        assert!(backups_to_prune(&[], 5).is_empty());
    }

    #[test]
    fn prune_all_when_keep_is_zero() {
        let existing = vec!["hibi-a.json".to_string(), "hibi-b.json".to_string()];
        assert_eq!(backups_to_prune(&existing, 0).len(), 2);
    }

    #[test]
    fn create_backup_copies_then_prunes() {
        let base = std::env::temp_dir().join("hibi-test-create-backup");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let db_path = base.join("hibi.json");
        fs::write(&db_path, "{}").unwrap();
        let backups = base.join("backups");

        create_backup(&db_path, &backups, "2026-01-01T00-00-00", 2).unwrap();
        create_backup(&db_path, &backups, "2026-01-02T00-00-00", 2).unwrap();
        create_backup(&db_path, &backups, "2026-01-03T00-00-00", 2).unwrap();

        let mut names = list_backups(&backups).unwrap();
        names.sort();
        assert_eq!(
            names,
            vec![
                "hibi-2026-01-02T00-00-00.json".to_string(),
                "hibi-2026-01-03T00-00-00.json".to_string(),
            ]
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn restore_snapshots_current_before_overwriting() {
        let base = std::env::temp_dir().join("hibi-test-restore");
        let _ = fs::remove_dir_all(&base);
        let backups = base.join("backups");
        fs::create_dir_all(&backups).unwrap();
        let db_path = base.join("hibi.json");

        fs::write(backups.join("hibi-2026-01-01T00-00-00.json"), "OLD").unwrap();
        fs::write(&db_path, "CURRENT").unwrap();

        restore_backup(
            &db_path,
            &backups,
            "hibi-2026-01-01T00-00-00.json",
            "2026-06-01T00-00-00",
            10,
        )
        .unwrap();

        assert_eq!(fs::read_to_string(&db_path).unwrap(), "OLD");
        assert_eq!(
            fs::read_to_string(backups.join("hibi-2026-06-01T00-00-00.json")).unwrap(),
            "CURRENT"
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn clock_total_adds_running_segment() {
        assert_eq!(clock_total_seconds(100, Some(700), 1000), 400);
    }

    #[test]
    fn clock_total_paused_is_just_accumulated() {
        assert_eq!(clock_total_seconds(250, None, 9999), 250);
    }

    #[test]
    fn clock_total_clamps_backwards_clock() {
        assert_eq!(clock_total_seconds(50, Some(1000), 900), 50);
    }

    #[test]
    fn minutes_floor_truncates_partial_minutes() {
        assert_eq!(seconds_to_minutes_floor(59), 0);
        assert_eq!(seconds_to_minutes_floor(60), 1);
        assert_eq!(seconds_to_minutes_floor(149), 2);
    }
}
