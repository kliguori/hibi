use crate::config::Config;
use crate::store::{Database, Mode, Session, Source, Type};
use chrono::{Local, NaiveDate};
use skim::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Cursor, Write};
use std::path::Path;

fn next_id(db: &mut Database) -> i64 {
    let id = db.next_id;
    db.next_id += 1;
    id
}

/// Fuzzy-pick one of `lines`; None on Esc/Ctrl-C.
fn skim_pick(lines: &[String]) -> Option<String> {
    let input = lines.join("\n");
    let options = SkimOptionsBuilder::default()
        .height(Some("100%"))
        .reverse(true)
        .multi(false)
        .build()
        .expect("valid skim options");

    let item_reader = SkimItemReader::default();
    let items = item_reader.of_bufread(Cursor::new(input));

    let output = Skim::run_with(&options, Some(items))?;
    if output.is_abort {
        return None;
    }
    output
        .selected_items
        .first()
        .map(|item| item.output().to_string())
}

pub fn type_add(db: &mut Database, name: &str) -> anyhow::Result<()> {
    let exists = db.types.iter().any(|t| t.name.eq_ignore_ascii_case(name));
    if exists {
        println!("Type '{}' already exists.", name);
        return Ok(());
    }
    let id = next_id(db);
    db.types.push(Type {
        id,
        name: name.to_string(),
    });
    println!("Added type '{}' (id {}).", name, id);
    Ok(())
}

pub fn type_list(db: &Database) -> anyhow::Result<()> {
    if db.types.is_empty() {
        println!("No types yet.");
        return Ok(());
    }
    for t in &db.types {
        println!("{:>4}  {}", t.id, t.name);
    }
    Ok(())
}

pub fn type_rm(db: &mut Database) -> anyhow::Result<()> {
    let Some((type_id, name)) = pick_type(db)? else {
        return Ok(());
    };
    let count = db.sources.iter().filter(|s| s.type_id == type_id).count();
    if count > 0 {
        println!(
            "Cannot remove type '{}' because {} source(s) reference it.",
            name, count
        );
        return Ok(());
    }
    db.types.retain(|t| t.id != type_id);
    println!("Type '{}' has been removed.", name);
    Ok(())
}

pub fn type_edit(db: &mut Database) -> anyhow::Result<()> {
    let Some((type_id, old_name)) = pick_type(db)? else {
        return Ok(());
    };
    let new_name = prompt_line("New name: ")?;
    if new_name.is_empty() {
        println!("No new name entered; nothing changed.");
        return Ok(());
    }
    if db
        .types
        .iter()
        .any(|t| t.id != type_id && t.name.eq_ignore_ascii_case(&new_name))
    {
        println!("Type '{}' already exists.", new_name);
        return Ok(());
    }
    if let Some(the_type) = db.types.iter_mut().find(|t| t.id == type_id) {
        the_type.name = new_name.clone();
    }
    println!("Renamed type '{}' to '{}'.", old_name, new_name);
    Ok(())
}

pub fn mode_add(db: &mut Database, name: &str) -> anyhow::Result<()> {
    let exists = db.modes.iter().any(|m| m.name.eq_ignore_ascii_case(name));
    if exists {
        println!("Mode '{}' already exists.", name);
        return Ok(());
    }
    let id = next_id(db);
    db.modes.push(Mode {
        id,
        name: name.to_string(),
    });
    println!("Added mode '{}' (id {}).", name, id);
    Ok(())
}

pub fn mode_list(db: &Database) -> anyhow::Result<()> {
    if db.modes.is_empty() {
        println!("No modes yet.");
        return Ok(());
    }
    for m in &db.modes {
        println!("{:>4}  {}", m.id, m.name);
    }
    Ok(())
}

pub fn mode_rm(db: &mut Database) -> anyhow::Result<()> {
    let Some((mode_id, name)) = pick_mode(db)? else {
        return Ok(());
    };
    let count = db.sessions.iter().filter(|s| s.mode_id == mode_id).count();
    if count > 0 {
        println!(
            "Cannot remove mode '{}' because {} session(s) reference it.",
            name, count
        );
        return Ok(());
    }
    db.modes.retain(|m| m.id != mode_id);
    println!("Mode '{}' has been removed.", name);
    Ok(())
}

pub fn mode_edit(db: &mut Database) -> anyhow::Result<()> {
    let Some((mode_id, old_name)) = pick_mode(db)? else {
        return Ok(());
    };
    let new_name = prompt_line("New name: ")?;
    if new_name.is_empty() {
        println!("No new name entered; nothing changed.");
        return Ok(());
    }
    if db
        .modes
        .iter()
        .any(|m| m.id != mode_id && m.name.eq_ignore_ascii_case(&new_name))
    {
        println!("Mode '{}' already exists.", new_name);
        return Ok(());
    }
    if let Some(the_mode) = db.modes.iter_mut().find(|m| m.id == mode_id) {
        the_mode.name = new_name.clone();
    }
    println!("Renamed mode '{}' to '{}'.", old_name, new_name);
    Ok(())
}

pub fn source_add(db: &mut Database, name: &str) -> anyhow::Result<()> {
    if db.sources.iter().any(|s| s.name.eq_ignore_ascii_case(name)) {
        println!("Source '{}' already exists.", name);
        return Ok(());
    }

    let Some((type_id, type_name)) = pick_type(db)? else {
        return Ok(());
    };

    let id = next_id(db);
    db.sources.push(Source {
        id,
        name: name.to_string(),
        type_id,
    });
    println!("Added source '{}' (type '{}', id {}).", name, type_name, id);
    Ok(())
}

pub fn source_list(db: &Database) -> anyhow::Result<()> {
    if db.sources.is_empty() {
        println!("No sources yet.");
        return Ok(());
    }
    for s in &db.sources {
        let type_name = db
            .types
            .iter()
            .find(|t| t.id == s.type_id)
            .map(|t| t.name.as_str())
            .unwrap_or("(unknown type)");
        println!("{:>4}  {}  [{}]", s.id, s.name, type_name);
    }
    Ok(())
}

pub fn source_rm(db: &mut Database) -> anyhow::Result<()> {
    let Some((source_id, name)) = pick_source(db)? else {
        return Ok(());
    };
    let count = db
        .sessions
        .iter()
        .filter(|s| s.source_id == source_id)
        .count();
    if count > 0 {
        println!(
            "Cannot remove source '{}' because {} session(s) reference it.",
            name, count
        );
        return Ok(());
    }
    db.sources.retain(|s| s.id != source_id);
    println!("Source '{}' has been removed.", name);
    Ok(())
}

pub fn source_edit(db: &mut Database) -> anyhow::Result<()> {
    let Some((source_id, _)) = pick_source(db)? else {
        return Ok(());
    };

    // Menu loop: change a field, then back to the menu until Finish.
    loop {
        let Some(source) = db.sources.iter().find(|s| s.id == source_id) else {
            return Ok(());
        };
        let cur_name = source.name.clone();
        let cur_type = db
            .types
            .iter()
            .find(|t| t.id == source.type_id)
            .map(|t| t.name.clone())
            .unwrap_or_else(|| "(unknown type)".to_string());

        let opt_name = format!("Name  [{}]", cur_name);
        let opt_type = format!("Type  [{}]", cur_type);
        let opt_done = "Finish".to_string();
        let Some(choice) = skim_pick(&[opt_name.clone(), opt_type.clone(), opt_done.clone()])
        else {
            break;
        };

        if choice == opt_done {
            break;
        } else if choice == opt_name {
            let new_name = prompt_line("New name: ")?;
            if new_name.is_empty() {
                println!("No new name entered.");
                continue;
            }
            if db
                .sources
                .iter()
                .any(|s| s.id != source_id && s.name.eq_ignore_ascii_case(&new_name))
            {
                println!("Source '{}' already exists.", new_name);
                continue;
            }
            if let Some(s) = db.sources.iter_mut().find(|s| s.id == source_id) {
                s.name = new_name.clone();
            }
            println!("Renamed to '{}'.", new_name);
        } else if choice == opt_type {
            let Some((type_id, type_name)) = pick_type(db)? else {
                continue;
            };
            if let Some(s) = db.sources.iter_mut().find(|s| s.id == source_id) {
                s.type_id = type_id;
            }
            println!("Type set to '{}'.", type_name);
        }
    }
    Ok(())
}

pub fn session_list(db: &Database) -> anyhow::Result<()> {
    if db.sessions.is_empty() {
        println!("No sessions yet.");
        return Ok(());
    }
    let mut sessions: Vec<&Session> = db.sessions.iter().collect();
    sessions.sort_by(|a, b| b.date.cmp(&a.date));

    for s in sessions {
        let source_name = db
            .sources
            .iter()
            .find(|src| src.id == s.source_id)
            .map(|src| src.name.as_str())
            .unwrap_or("(unknown source)");
        let mode_name = db
            .modes
            .iter()
            .find(|m| m.id == s.mode_id)
            .map(|m| m.name.as_str())
            .unwrap_or("(unknown mode)");
        println!(
            "{}  {} · {} · {}m",
            s.date, source_name, mode_name, s.minutes
        );
    }
    Ok(())
}

pub fn session_rm(db: &mut Database) -> anyhow::Result<()> {
    if db.sessions.is_empty() {
        println!("No sessions to remove.");
        return Ok(());
    }

    // Prefix each line with the id + a tab so we can recover the pick.
    let lines: Vec<String> = db
        .sessions
        .iter()
        .map(|s| {
            let source_name = db
                .sources
                .iter()
                .find(|src| src.id == s.source_id)
                .map(|src| src.name.as_str())
                .unwrap_or("(unknown source)");
            let mode_name = db
                .modes
                .iter()
                .find(|m| m.id == s.mode_id)
                .map(|m| m.name.as_str())
                .unwrap_or("(unknown mode)");
            format!(
                "{}\t{}  {} · {} · {}m",
                s.id, s.date, source_name, mode_name, s.minutes
            )
        })
        .collect();

    let Some(chosen) = skim_pick(&lines) else {
        return Ok(());
    };

    let id: i64 = chosen
        .split('\t')
        .next()
        .and_then(|field| field.parse().ok())
        .expect("skim line always starts with the session id we wrote");

    db.sessions.retain(|s| s.id != id);
    println!("Removed session {}.", id);
    Ok(())
}

pub fn session_add(db: &mut Database) -> anyhow::Result<()> {
    let Some((source_id, source_name)) = pick_source(db)? else {
        return Ok(());
    };
    let Some((mode_id, mode_name)) = pick_mode(db)? else {
        return Ok(());
    };
    let minutes = prompt_minutes()?;
    let date = prompt_date()?;

    let id = next_id(db);
    db.sessions.push(Session {
        id,
        source_id,
        mode_id,
        minutes,
        date: date.clone(),
    });
    println!(
        "Added {}m of {} · {} on {}.",
        minutes, source_name, mode_name, date
    );
    Ok(())
}

pub fn session_edit(db: &mut Database) -> anyhow::Result<()> {
    if db.sessions.is_empty() {
        println!("No sessions to edit.");
        return Ok(());
    }

    let lines: Vec<String> = db
        .sessions
        .iter()
        .map(|s| {
            let source_name = db
                .sources
                .iter()
                .find(|src| src.id == s.source_id)
                .map(|src| src.name.as_str())
                .unwrap_or("(unknown source)");
            let mode_name = db
                .modes
                .iter()
                .find(|m| m.id == s.mode_id)
                .map(|m| m.name.as_str())
                .unwrap_or("(unknown mode)");
            format!(
                "{}\t{}  {} · {} · {}m",
                s.id, s.date, source_name, mode_name, s.minutes
            )
        })
        .collect();
    let Some(chosen) = skim_pick(&lines) else {
        return Ok(());
    };
    let id: i64 = chosen
        .split('\t')
        .next()
        .and_then(|field| field.parse().ok())
        .expect("skim line always starts with the session id we wrote");

    // Menu loop: change a field, then back to the menu until Finish.
    loop {
        let Some(session) = db.sessions.iter().find(|s| s.id == id) else {
            return Ok(());
        };
        let cur_source = db
            .sources
            .iter()
            .find(|src| src.id == session.source_id)
            .map(|src| src.name.clone())
            .unwrap_or_else(|| "(unknown source)".to_string());
        let cur_mode = db
            .modes
            .iter()
            .find(|m| m.id == session.mode_id)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "(unknown mode)".to_string());
        let cur_minutes = session.minutes;
        let cur_date = session.date.clone();

        let opt_source = format!("Source   [{}]", cur_source);
        let opt_mode = format!("Mode     [{}]", cur_mode);
        let opt_minutes = format!("Minutes  [{}]", cur_minutes);
        let opt_date = format!("Date     [{}]", cur_date);
        let opt_done = "Finish".to_string();
        let Some(choice) = skim_pick(&[
            opt_source.clone(),
            opt_mode.clone(),
            opt_minutes.clone(),
            opt_date.clone(),
            opt_done.clone(),
        ]) else {
            break;
        };

        if choice == opt_done {
            break;
        } else if choice == opt_source {
            let Some((source_id, name)) = pick_source(db)? else {
                continue;
            };
            if let Some(s) = db.sessions.iter_mut().find(|s| s.id == id) {
                s.source_id = source_id;
            }
            println!("Source set to '{}'.", name);
        } else if choice == opt_mode {
            let Some((mode_id, name)) = pick_mode(db)? else {
                continue;
            };
            if let Some(s) = db.sessions.iter_mut().find(|s| s.id == id) {
                s.mode_id = mode_id;
            }
            println!("Mode set to '{}'.", name);
        } else if choice == opt_minutes {
            let minutes = prompt_minutes()?;
            if let Some(s) = db.sessions.iter_mut().find(|s| s.id == id) {
                s.minutes = minutes;
            }
            println!("Minutes set to {}.", minutes);
        } else if choice == opt_date {
            let date = prompt_date()?;
            if let Some(s) = db.sessions.iter_mut().find(|s| s.id == id) {
                s.date = date.clone();
            }
            println!("Date set to {}.", date);
        }
    }
    Ok(())
}

/// Fuzzy-pick a source as (id, name).
fn pick_source(db: &Database) -> anyhow::Result<Option<(i64, String)>> {
    if db.sources.is_empty() {
        println!("No sources yet. Add one first with `hibi source add`.");
        return Ok(None);
    }
    let names: Vec<String> = db.sources.iter().map(|s| s.name.clone()).collect();
    let Some(name) = skim_pick(&names) else {
        return Ok(None);
    };
    Ok(db
        .sources
        .iter()
        .find(|s| s.name == name)
        .map(|s| (s.id, s.name.clone())))
}

/// Fuzzy-pick a mode as (id, name).
fn pick_mode(db: &Database) -> anyhow::Result<Option<(i64, String)>> {
    if db.modes.is_empty() {
        println!("No modes yet. Add one first with `hibi mode add`.");
        return Ok(None);
    }
    let names: Vec<String> = db.modes.iter().map(|m| m.name.clone()).collect();
    let Some(name) = skim_pick(&names) else {
        return Ok(None);
    };
    Ok(db
        .modes
        .iter()
        .find(|m| m.name == name)
        .map(|m| (m.id, m.name.clone())))
}

/// Fuzzy-pick a type as (id, name).
fn pick_type(db: &Database) -> anyhow::Result<Option<(i64, String)>> {
    if db.types.is_empty() {
        println!("No types yet. Add one first with `hibi type add`.");
        return Ok(None);
    }
    let names: Vec<String> = db.types.iter().map(|t| t.name.clone()).collect();
    let Some(name) = skim_pick(&names) else {
        return Ok(None);
    };
    Ok(db
        .types
        .iter()
        .find(|t| t.name == name)
        .map(|t| (t.id, t.name.clone())))
}

fn prompt_line(label: &str) -> anyhow::Result<String> {
    print!("{}", label);
    io::stdout().flush()?; // so the prompt shows before read_line blocks
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn prompt_minutes() -> anyhow::Result<i64> {
    Ok(prompt_line("Minutes: ")?.parse()?)
}

/// Prompt for a date; blank means today. Non-blank is validated as YYYY-MM-DD.
fn prompt_date() -> anyhow::Result<String> {
    let input = prompt_line("Date [YYYY-MM-DD, blank = today]: ")?;
    if input.is_empty() {
        Ok(Local::now().format("%Y-%m-%d").to_string())
    } else {
        NaiveDate::parse_from_str(&input, "%Y-%m-%d")?;
        Ok(input)
    }
}

pub fn log(db: &mut Database) -> anyhow::Result<()> {
    let Some((source_id, source_name)) = pick_source(db)? else {
        return Ok(());
    };
    let Some((mode_id, mode_name)) = pick_mode(db)? else {
        return Ok(());
    };
    let minutes = prompt_minutes()?;
    let date = Local::now().format("%Y-%m-%d").to_string();

    let id = next_id(db);
    db.sessions.push(Session {
        id,
        source_id,
        mode_id,
        minutes,
        date: date.clone(),
    });
    println!(
        "Logged {}m of {} · {} on {}.",
        minutes, source_name, mode_name, date
    );
    Ok(())
}

fn to_unix(rfc3339: &str) -> anyhow::Result<i64> {
    Ok(chrono::DateTime::parse_from_rfc3339(rfc3339)?.timestamp())
}

fn to_local_date(rfc3339: &str) -> anyhow::Result<String> {
    let dt = chrono::DateTime::parse_from_rfc3339(rfc3339)?.with_timezone(&Local);
    Ok(dt.format("%Y-%m-%d").to_string())
}

fn clock_elapsed(clock: &crate::store::Clock, now_unix: i64) -> anyhow::Result<i64> {
    let running_since = match &clock.running_since {
        Some(ts) => Some(to_unix(ts)?),
        None => None,
    };
    Ok(crate::store::clock_total_seconds(
        clock.accumulated_seconds,
        running_since,
        now_unix,
    ))
}

pub fn clock_in(root: &Path, target_lang: &str) -> anyhow::Result<()> {
    let clock_path = crate::store::clock_file(root);
    if let Some(active) = crate::store::load_clock(&clock_path)? {
        println!(
            "A timer is already running for '{}'. Use `hibi clock status`, `clock out`, or `clock cancel`.",
            active.language
        );
        return Ok(());
    }

    let db = crate::store::load(&crate::store::db_file(root, target_lang))?;
    let Some((source_id, _)) = pick_source(&db)? else {
        return Ok(());
    };
    let Some((mode_id, _)) = pick_mode(&db)? else {
        return Ok(());
    };

    let now = Local::now().to_rfc3339();
    let clock = crate::store::Clock {
        language: target_lang.to_string(),
        source_id,
        mode_id,
        started_at: now.clone(),
        accumulated_seconds: 0,
        running_since: Some(now),
    };
    crate::store::save_clock(&clock_path, &clock)?;
    println!(
        "Clocked in on '{}'. Run `hibi clock out` when you're done.",
        target_lang
    );
    Ok(())
}

pub fn clock_out(root: &Path, keep_backups: usize) -> anyhow::Result<()> {
    let clock_path = crate::store::clock_file(root);
    let Some(clock) = crate::store::load_clock(&clock_path)? else {
        println!("No active timer. Start one with `hibi clock in`.");
        return Ok(());
    };

    let total_seconds = clock_elapsed(&clock, Local::now().timestamp())?;
    let minutes = crate::store::seconds_to_minutes_floor(total_seconds);

    // Under a minute rounds to nothing — don't log a 0m session.
    if minutes < 1 {
        crate::store::clear_clock(&clock_path)?;
        println!(
            "Under a minute ({}s) — nothing logged. Timer cleared.",
            total_seconds
        );
        return Ok(());
    }

    // Log into the timer's own language, not whatever is currently selected.
    let mut db = crate::store::load(&crate::store::db_file(root, &clock.language))?;

    // Source/mode removed mid-timer: log anyway rather than lose the time.
    if !db.sources.iter().any(|s| s.id == clock.source_id)
        || !db.modes.iter().any(|m| m.id == clock.mode_id)
    {
        println!("Warning: this timer's source or mode was removed — logging it as unknown.");
    }

    let date = to_local_date(&clock.started_at)?;
    let id = next_id(&mut db);
    db.sessions.push(Session {
        id,
        source_id: clock.source_id,
        mode_id: clock.mode_id,
        minutes,
        date: date.clone(),
    });

    let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    crate::store::commit(root, &clock.language, &db, &timestamp, keep_backups)?;
    crate::store::clear_clock(&clock_path)?;

    let source_name = db
        .sources
        .iter()
        .find(|s| s.id == clock.source_id)
        .map(|s| s.name.as_str())
        .unwrap_or("(unknown source)");
    let mode_name = db
        .modes
        .iter()
        .find(|m| m.id == clock.mode_id)
        .map(|m| m.name.as_str())
        .unwrap_or("(unknown mode)");
    println!(
        "Clocked out. Logged {}m of {} · {} on {} ({}).",
        minutes, source_name, mode_name, date, clock.language
    );
    Ok(())
}

pub fn clock_pause(root: &Path) -> anyhow::Result<()> {
    let clock_path = crate::store::clock_file(root);
    let Some(mut clock) = crate::store::load_clock(&clock_path)? else {
        println!("No active timer.");
        return Ok(());
    };
    if clock.running_since.is_none() {
        println!("Timer is already paused.");
        return Ok(());
    }
    // Fold the running segment into the accumulator, then stop it.
    clock.accumulated_seconds = clock_elapsed(&clock, Local::now().timestamp())?;
    clock.running_since = None;
    crate::store::save_clock(&clock_path, &clock)?;
    println!("Paused. Resume with `hibi clock resume`.");
    Ok(())
}

pub fn clock_resume(root: &Path) -> anyhow::Result<()> {
    let clock_path = crate::store::clock_file(root);
    let Some(mut clock) = crate::store::load_clock(&clock_path)? else {
        println!("No active timer.");
        return Ok(());
    };
    if clock.running_since.is_some() {
        println!("Timer is already running.");
        return Ok(());
    }
    clock.running_since = Some(Local::now().to_rfc3339());
    crate::store::save_clock(&clock_path, &clock)?;
    println!("Resumed.");
    Ok(())
}

pub fn clock_status(root: &Path) -> anyhow::Result<()> {
    let clock_path = crate::store::clock_file(root);
    let Some(clock) = crate::store::load_clock(&clock_path)? else {
        println!("No active timer.");
        return Ok(());
    };
    let total = clock_elapsed(&clock, Local::now().timestamp())?;
    let db = crate::store::load(&crate::store::db_file(root, &clock.language))?;
    let source_name = db
        .sources
        .iter()
        .find(|s| s.id == clock.source_id)
        .map(|s| s.name.as_str())
        .unwrap_or("(unknown source)");
    let mode_name = db
        .modes
        .iter()
        .find(|m| m.id == clock.mode_id)
        .map(|m| m.name.as_str())
        .unwrap_or("(unknown mode)");
    let state = if clock.running_since.is_some() {
        "running"
    } else {
        "paused"
    };
    println!(
        "{}: {} · {} — {} ({}m {}s elapsed)",
        clock.language,
        source_name,
        mode_name,
        state,
        total / 60,
        total % 60
    );
    Ok(())
}

pub fn clock_cancel(root: &Path) -> anyhow::Result<()> {
    let clock_path = crate::store::clock_file(root);
    if crate::store::load_clock(&clock_path)?.is_none() {
        println!("No active timer to cancel.");
        return Ok(());
    }
    crate::store::clear_clock(&clock_path)?;
    println!("Timer cancelled. Nothing logged.");
    Ok(())
}

/// Warn (on stderr) if a timer is running for a language other than `lang`.
pub fn warn_if_other_timer(root: &Path, lang: &str) -> anyhow::Result<()> {
    if let Some(clock) = crate::store::load_clock(&crate::store::clock_file(root))? {
        if clock.language != lang {
            eprintln!("Note: a clock timer is running for '{}'.", clock.language);
        }
    }
    Ok(())
}

// The helpers below take an explicit `today` rather than reading the clock, so
// they're pure and the tests can pin the date. Dates are "YYYY-MM-DD" strings,
// which sort chronologically, so window filtering is plain string comparison.

fn day_total(db: &Database, day: NaiveDate) -> i64 {
    let key = day.format("%Y-%m-%d").to_string();
    db.sessions
        .iter()
        .filter(|s| s.date == key)
        .map(|s| s.minutes)
        .sum()
}

/// Rolling window of `days` days ending on `today`, inclusive.
fn window_total(db: &Database, today: NaiveDate, days: i64) -> i64 {
    let start = today - chrono::Duration::days(days - 1);
    let start_key = start.format("%Y-%m-%d").to_string();
    let end_key = today.format("%Y-%m-%d").to_string();
    db.sessions
        .iter()
        .filter(|s| s.date >= start_key && s.date <= end_key)
        .map(|s| s.minutes)
        .sum()
}

fn all_time_total(db: &Database) -> i64 {
    db.sessions.iter().map(|s| s.minutes).sum()
}

/// Consecutive logged days counting back from `today`; 0 if nothing today.
fn current_streak(db: &Database, today: NaiveDate) -> i64 {
    let logged: HashSet<String> = db.sessions.iter().map(|s| s.date.clone()).collect();
    let mut streak = 0;
    let mut day = today;
    while logged.contains(&day.format("%Y-%m-%d").to_string()) {
        streak += 1;
        day = day - chrono::Duration::days(1);
    }
    streak
}

fn longest_streak(db: &Database) -> i64 {
    let mut days: Vec<NaiveDate> = db
        .sessions
        .iter()
        .filter_map(|s| NaiveDate::parse_from_str(&s.date, "%Y-%m-%d").ok())
        .collect();
    days.sort();
    days.dedup();

    let mut best = 0;
    let mut run = 0;
    let mut prev: Option<NaiveDate> = None;
    for d in days {
        run = match prev {
            Some(p) if d == p + chrono::Duration::days(1) => run + 1,
            _ => 1,
        };
        best = best.max(run);
        prev = Some(d);
    }
    best
}

/// Sort rows by minutes descending, ties broken by name for stable output.
fn sort_desc(rows: &mut [(String, i64)]) {
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
}

fn totals_by_type(db: &Database) -> Vec<(String, i64)> {
    let mut totals: HashMap<i64, i64> = HashMap::new();
    for s in &db.sessions {
        if let Some(source) = db.sources.iter().find(|src| src.id == s.source_id) {
            *totals.entry(source.type_id).or_insert(0) += s.minutes;
        }
    }
    let mut rows: Vec<(String, i64)> = totals
        .into_iter()
        .map(|(type_id, minutes)| {
            let name = db
                .types
                .iter()
                .find(|t| t.id == type_id)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| "(unknown type)".to_string());
            (name, minutes)
        })
        .collect();
    sort_desc(&mut rows);
    rows
}

fn totals_by_mode(db: &Database) -> Vec<(String, i64)> {
    let mut totals: HashMap<i64, i64> = HashMap::new();
    for s in &db.sessions {
        *totals.entry(s.mode_id).or_insert(0) += s.minutes;
    }
    let mut rows: Vec<(String, i64)> = totals
        .into_iter()
        .map(|(mode_id, minutes)| {
            let name = db
                .modes
                .iter()
                .find(|m| m.id == mode_id)
                .map(|m| m.name.clone())
                .unwrap_or_else(|| "(unknown mode)".to_string());
            (name, minutes)
        })
        .collect();
    sort_desc(&mut rows);
    rows
}

fn totals_by_source(db: &Database) -> Vec<(String, i64)> {
    let mut totals: HashMap<i64, i64> = HashMap::new();
    for s in &db.sessions {
        *totals.entry(s.source_id).or_insert(0) += s.minutes;
    }
    let mut rows: Vec<(String, i64)> = totals
        .into_iter()
        .map(|(source_id, minutes)| {
            let name = db
                .sources
                .iter()
                .find(|src| src.id == source_id)
                .map(|src| src.name.clone())
                .unwrap_or_else(|| "(unknown source)".to_string());
            (name, minutes)
        })
        .collect();
    sort_desc(&mut rows);
    rows
}

/// Render minutes as "3h 30m" / "45m" / "12h".
fn fmt_hm(minutes: i64) -> String {
    let hours = minutes / 60;
    let mins = minutes % 60;
    match (hours, mins) {
        (0, m) => format!("{}m", m),
        (h, 0) => format!("{}h", h),
        (h, m) => format!("{}h {}m", h, m),
    }
}

/// Proportional bar of block chars; empty when `max` is non-positive.
fn bar(value: i64, max: i64, width: usize) -> String {
    if max <= 0 {
        return String::new();
    }
    let filled = ((value as f64 / max as f64) * width as f64).round() as usize;
    "█".repeat(filled)
}

fn print_breakdown(title: &str, rows: &[(String, i64)]) {
    if rows.is_empty() {
        return;
    }
    let total: i64 = rows.iter().map(|(_, m)| m).sum();
    let max = rows.iter().map(|(_, m)| *m).max().unwrap_or(0);
    let name_width = rows.iter().map(|(n, _)| n.len()).max().unwrap_or(0);

    println!();
    println!("{}", title);
    for (name, minutes) in rows {
        let pct = if total > 0 {
            (*minutes as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        println!(
            "  {:<name_width$}  {:<12}  {:>7}  {:>3.0}%",
            name,
            bar(*minutes, max, 12),
            fmt_hm(*minutes),
            pct,
        );
    }
}

pub fn stats(db: &Database) -> anyhow::Result<()> {
    if db.sessions.is_empty() {
        println!("No sessions yet — nothing to report.");
        return Ok(());
    }

    let today = Local::now().date_naive();
    let yesterday = today - chrono::Duration::days(1);

    println!("=== hibi stats ===");
    println!();
    println!("  {:<13}{:>10}{:>10}", "", "total", "avg/day");

    let rows: [(&str, i64, Option<i64>); 7] = [
        ("today", day_total(db, today), None),
        ("yesterday", day_total(db, yesterday), None),
        ("last 7 days", window_total(db, today, 7), Some(7)),
        ("last 30 days", window_total(db, today, 30), Some(30)),
        ("last 90 days", window_total(db, today, 90), Some(90)),
        ("last 360 days", window_total(db, today, 360), Some(360)),
        ("all time", all_time_total(db), None),
    ];
    for (label, total, avg_over) in rows {
        let avg = match avg_over {
            Some(days) => fmt_hm(total / days),
            None => "—".to_string(),
        };
        println!("  {:<13}{:>10}{:>10}", label, fmt_hm(total), avg);
    }

    println!();
    println!(
        "  streak: {} days  (longest {})",
        current_streak(db, today),
        longest_streak(db)
    );

    print_breakdown("By type", &totals_by_type(db));
    print_breakdown("By mode", &totals_by_mode(db));
    print_breakdown("By source", &totals_by_source(db));

    Ok(())
}

/// Create a new language dataset and switch to it.
pub fn language_add(config: &mut Config, name: &str) -> anyhow::Result<()> {
    if name == crate::store::SAMPLE_LANG {
        println!("'{}' is a reserved dataset name.", name);
        return Ok(());
    }
    let root = crate::store::data_root()?;
    if crate::store::language_dir(&root, name).exists() {
        println!("Language '{}' already exists.", name);
        return Ok(());
    }
    fs::create_dir_all(crate::store::backups_dir(&root, name))?;
    crate::store::save(&crate::store::db_file(&root, name), &Database::default())?;

    config.current = name.to_string();
    crate::config::save(config)?;
    println!("Created language '{}' and switched to it.", name);
    Ok(())
}

pub fn language_list(config: &Config) -> anyhow::Result<()> {
    let root = crate::store::data_root()?;
    let mut names: Vec<String> = Vec::new();
    if root.exists() {
        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    names.push(name.to_string());
                }
            }
        }
    }
    if names.is_empty() {
        println!("No languages yet. Add one with `hibi language add <name>`.");
        return Ok(());
    }
    names.sort();
    for name in names {
        let marker = if name == config.current { "*" } else { " " };
        println!("{} {}", marker, name);
    }
    Ok(())
}

pub fn language_use(config: &mut Config, name: &str) -> anyhow::Result<()> {
    let root = crate::store::data_root()?;
    if !crate::store::language_dir(&root, name).exists() {
        println!(
            "Language '{}' does not exist. Create it with `hibi language add {}`.",
            name, name
        );
        return Ok(());
    }
    config.current = name.to_string();
    crate::config::save(config)?;
    println!("Switched to '{}'.", name);
    Ok(())
}

pub fn config_show(config: &Config) -> anyhow::Result<()> {
    let root = crate::store::data_root()?;
    println!("current dataset : {}", config.current);
    println!("keep backups    : {}", config.keep_backups);
    println!("data root       : {}", root.display());
    println!(
        "config file     : {}",
        crate::store::config_file()?.display()
    );
    Ok(())
}

pub fn config_set_keep(config: &mut Config, count: usize) -> anyhow::Result<()> {
    config.keep_backups = count;
    crate::config::save(config)?;
    println!("Will keep {} backup(s) per dataset.", count);
    Ok(())
}

fn active_language(config: &Config, lang_override: Option<String>) -> String {
    lang_override.unwrap_or_else(|| config.current.clone())
}

pub fn backup_list(config: &Config, lang_override: Option<String>) -> anyhow::Result<()> {
    let root = crate::store::data_root()?;
    let lang = active_language(config, lang_override);
    let mut names = crate::store::list_backups(&crate::store::backups_dir(&root, &lang))?;
    if names.is_empty() {
        println!("No backups for '{}' yet.", lang);
        return Ok(());
    }
    names.sort();
    names.reverse(); // newest first
    println!("Backups for '{}':", lang);
    for name in names {
        println!("  {}", name);
    }
    Ok(())
}

/// Fuzzy-pick a backup and restore from it. The current state is snapshotted
/// first, so a wrong choice can be undone.
pub fn backup_restore(config: &Config, lang_override: Option<String>) -> anyhow::Result<()> {
    let root = crate::store::data_root()?;
    let lang = active_language(config, lang_override);
    let backups_dir = crate::store::backups_dir(&root, &lang);

    let mut names = crate::store::list_backups(&backups_dir)?;
    if names.is_empty() {
        println!("No backups for '{}' to restore.", lang);
        return Ok(());
    }
    names.sort();
    names.reverse(); // newest first in the picker

    let Some(chosen) = skim_pick(&names) else {
        return Ok(());
    };

    let db_path = crate::store::db_file(&root, &lang);
    let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    crate::store::restore_backup(
        &db_path,
        &backups_dir,
        &chosen,
        &timestamp,
        config.keep_backups,
    )?;

    println!(
        "Restored '{}' from {}.\nYour pre-restore state was saved as a new backup — \
         run `hibi backup restore` again to undo.",
        lang, chosen
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Database;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 7, 7).unwrap()
    }

    fn sample_db() -> Database {
        crate::store::sample_database()
    }

    #[test]
    fn day_totals_pick_a_single_day() {
        let db = sample_db();
        assert_eq!(day_total(&db, today()), 30);
        assert_eq!(day_total(&db, today() - chrono::Duration::days(1)), 20);
        assert_eq!(day_total(&db, today() - chrono::Duration::days(3)), 0);
    }

    #[test]
    fn rolling_windows_are_inclusive_and_cumulative() {
        let db = sample_db();
        let t = today();
        assert_eq!(window_total(&db, t, 7), 60); // 30 + 20 + 10
        assert_eq!(window_total(&db, t, 30), 100); // + 40
        assert_eq!(window_total(&db, t, 90), 160); // + 60
        assert_eq!(window_total(&db, t, 360), 210); // + 50
        assert_eq!(all_time_total(&db), 310); // + 100
    }

    #[test]
    fn current_streak_counts_back_from_today() {
        let db = sample_db();
        assert_eq!(current_streak(&db, today()), 3); // 07-05, 07-06, 07-07
        assert_eq!(current_streak(&db, today() + chrono::Duration::days(1)), 0);
    }

    #[test]
    fn longest_streak_finds_the_best_run() {
        let db = sample_db();
        assert_eq!(longest_streak(&db), 3);
    }

    #[test]
    fn breakdowns_group_and_sort_descending() {
        let db = sample_db();
        assert_eq!(
            totals_by_type(&db),
            vec![("youtube".to_string(), 200), ("podcast".to_string(), 110)]
        );
        assert_eq!(
            totals_by_mode(&db),
            vec![
                ("watching".to_string(), 200),
                ("listening".to_string(), 110)
            ]
        );
        assert_eq!(
            totals_by_source(&db),
            vec![("Show A".to_string(), 200), ("Pod B".to_string(), 110)]
        );
    }

    #[test]
    fn fmt_hm_formats_hours_and_minutes() {
        assert_eq!(fmt_hm(0), "0m");
        assert_eq!(fmt_hm(45), "45m");
        assert_eq!(fmt_hm(60), "1h");
        assert_eq!(fmt_hm(90), "1h 30m");
        assert_eq!(fmt_hm(200), "3h 20m");
    }

    #[test]
    fn bars_scale_to_max_and_guard_zero() {
        assert_eq!(bar(10, 10, 10), "█".repeat(10));
        assert_eq!(bar(5, 10, 10), "█".repeat(5));
        assert_eq!(bar(0, 10, 10), "");
        assert_eq!(bar(10, 0, 10), "");
    }

    #[test]
    fn empty_database_is_safe() {
        let db = Database::default();
        assert_eq!(all_time_total(&db), 0);
        assert_eq!(current_streak(&db, today()), 0);
        assert_eq!(longest_streak(&db), 0);
        assert!(totals_by_type(&db).is_empty());
    }

    // View the dashboard from the sample with no data on disk:
    //     cargo test dashboard_renders -- --nocapture
    #[test]
    fn dashboard_renders() {
        stats(&sample_db()).unwrap();
    }
}
