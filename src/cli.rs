use crate::config::Config;
use crate::store;
use clap::{Parser, Subcommand};

/// hibi — immersion logging tracker
#[derive(Parser)]
#[command(name = "hibi", version, about)]
pub struct Cli {
    /// Operate on a specific language/dataset for this command only.
    #[arg(long, global = true)]
    lang: Option<String>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
pub enum Command {
    /// Manage types (anime, podcast, novel, …)
    Type {
        #[command(subcommand)]
        action: TypeAction,
    },
    /// Manage modes (listening, reading, watching, …)
    Mode {
        #[command(subcommand)]
        action: ModeAction,
    },
    /// Manage sources (the things you immerse in)
    Source {
        #[command(subcommand)]
        action: SourceAction,
    },
    /// Manage sessions (logged immersion time)
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Log an immersion session (interactive)
    Log,
    /// Show immersion statistics
    Stats,
    /// Manage languages (separate datasets, one shared git history)
    Language {
        #[command(subcommand)]
        action: LanguageAction,
    },
    /// View or change configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Browse the change history or restore the active dataset from it
    Backup {
        #[command(subcommand)]
        action: BackupAction,
    },
    /// Time a session live with a clock in / out timer
    Clock {
        #[command(subcommand)]
        action: ClockAction,
    },
}
#[derive(Subcommand)]
pub enum TypeAction {
    /// Add a new type
    Add { name: String },
    /// List all types
    List,
    /// Remove a type (pick from a menu; only if unused)
    Rm,
    /// Rename a type (pick from a menu)
    Edit,
}
#[derive(Subcommand)]
pub enum ModeAction {
    /// Add a new mode
    Add { name: String },
    /// List all modes
    List,
    /// Remove a mode (pick from a menu; only if unused)
    Rm,
    /// Rename a mode (pick from a menu)
    Edit,
}
#[derive(Subcommand)]
pub enum SourceAction {
    /// Add a new source; pick its type from a menu: hibi source add "Terrace House"
    Add { name: String },
    /// List all sources
    List,
    /// Remove a source (pick from a menu; only if it has no sessions)
    Rm,
    /// Rename a source (pick from a menu)
    Edit,
}
#[derive(Subcommand)]
pub enum SessionAction {
    /// List logged sessions
    List,
    /// Add a session (interactive, with a date)
    Add,
    /// Edit a session (pick from a menu)
    Edit,
    /// Remove a session (pick from a menu)
    Rm,
}
#[derive(Subcommand)]
pub enum LanguageAction {
    /// Create a new language dataset
    Add { name: String },
    /// List languages (the current one is marked with *)
    List,
    /// Switch the current language
    Use { name: String },
}
#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show the current configuration and paths
    Show,
}
#[derive(Subcommand)]
pub enum BackupAction {
    /// Show the data root's change history (newest first)
    List,
    /// Restore the active dataset from a past commit (interactive)
    Restore,
}
#[derive(Subcommand)]
pub enum ClockAction {
    /// Start a timer: pick source + mode
    In,
    /// Stop the timer and log the elapsed session
    Out,
    /// Pause the running timer
    Pause,
    /// Resume a paused timer
    Resume,
    /// Show the running timer and elapsed time
    Status,
    /// Discard the running timer without logging
    Cancel,
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // First run: seed the sample dataset and make it current.
    if store::ensure_initialized()? {
        let mut initial = Config::default();
        initial.current = store::SAMPLE_LANG.to_string();
        crate::config::save(&initial)?;
        println!(
            "Welcome to hibi! You're on the read-only 'sample' dataset — try `hibi stats`.\n\
             Run `hibi help` for all commands, and `hibi language add <name>` to start your own."
        );
    }

    let mut config = crate::config::load()?;

    match cli.command {
        Command::Language { action } => match action {
            LanguageAction::Add { name } => crate::commands::language_add(&mut config, &name),
            LanguageAction::List => crate::commands::language_list(&config),
            LanguageAction::Use { name } => crate::commands::language_use(&mut config, &name),
        },
        Command::Config { action } => match action {
            ConfigAction::Show => crate::commands::config_show(&config),
        },
        Command::Backup { action } => match action {
            BackupAction::List => crate::commands::backup_list(&config, cli.lang),
            BackupAction::Restore => crate::commands::backup_restore(&config, cli.lang),
        },
        // Global timer — only `clock in` needs a target language.
        Command::Clock { action } => {
            let root = store::data_root()?;
            match action {
                ClockAction::In => {
                    let lang = cli.lang.unwrap_or_else(|| config.current.clone());
                    if lang == store::SAMPLE_LANG {
                        println!(
                            "The 'sample' dataset is read-only — clock in on your own language."
                        );
                        Ok(())
                    } else {
                        crate::commands::clock_in(&root, &lang)
                    }
                }
                ClockAction::Out => crate::commands::clock_out(&root),
                ClockAction::Pause => crate::commands::clock_pause(&root),
                ClockAction::Resume => crate::commands::clock_resume(&root),
                ClockAction::Status => crate::commands::clock_status(&root),
                ClockAction::Cancel => crate::commands::clock_cancel(&root),
            }
        }
        command => run_data_command(command, &config, cli.lang),
    }
}

/// Commands allowed against the read-only sample.
fn is_read_only(command: &Command) -> bool {
    matches!(
        command,
        Command::Stats
            | Command::Type {
                action: TypeAction::List
            }
            | Command::Mode {
                action: ModeAction::List
            }
            | Command::Source {
                action: SourceAction::List
            }
            | Command::Session {
                action: SessionAction::List
            }
    )
}

fn run_data_command(
    command: Command,
    config: &Config,
    lang_override: Option<String>,
) -> anyhow::Result<()> {
    let root = store::data_root()?;
    let lang = lang_override.unwrap_or_else(|| config.current.clone());

    if lang == store::SAMPLE_LANG && !is_read_only(&command) {
        println!(
            "The 'sample' dataset is read-only. Start your own with \
             `hibi language add <name>` then `hibi language use <name>`."
        );
        return Ok(());
    }

    crate::commands::warn_if_other_timer(&root, &lang)?;
    let db_path = store::db_file(&root, &lang);

    let mut db = store::load(&db_path)?;
    let before = store::snapshot(&db)?;
    let description = format!("{}: {}", lang, describe(&command));

    match command {
        Command::Type { action } => match action {
            TypeAction::Add { name } => crate::commands::type_add(&mut db, &name)?,
            TypeAction::List => crate::commands::type_list(&db)?,
            TypeAction::Rm => crate::commands::type_rm(&mut db)?,
            TypeAction::Edit => crate::commands::type_edit(&mut db)?,
        },
        Command::Mode { action } => match action {
            ModeAction::Add { name } => crate::commands::mode_add(&mut db, &name)?,
            ModeAction::List => crate::commands::mode_list(&db)?,
            ModeAction::Rm => crate::commands::mode_rm(&mut db)?,
            ModeAction::Edit => crate::commands::mode_edit(&mut db)?,
        },
        Command::Source { action } => match action {
            SourceAction::Add { name } => crate::commands::source_add(&mut db, &name)?,
            SourceAction::List => crate::commands::source_list(&db)?,
            SourceAction::Rm => crate::commands::source_rm(&mut db)?,
            SourceAction::Edit => crate::commands::source_edit(&mut db)?,
        },
        Command::Session { action } => match action {
            SessionAction::List => crate::commands::session_list(&db)?,
            SessionAction::Add => crate::commands::session_add(&mut db)?,
            SessionAction::Edit => crate::commands::session_edit(&mut db)?,
            SessionAction::Rm => crate::commands::session_rm(&mut db)?,
        },
        Command::Log => crate::commands::log(&mut db)?,
        Command::Stats => crate::commands::stats(&db)?,
        // Meta and clock commands were already handled in run().
        Command::Language { .. }
        | Command::Config { .. }
        | Command::Backup { .. }
        | Command::Clock { .. } => {
            unreachable!("meta and clock commands are handled in run()")
        }
    }

    let after = store::snapshot(&db)?;
    if after != before {
        store::commit(&root, &lang, &db, &description)?;
    }
    Ok(())
}

/// Commit-message note for a command. The `List` arms never fire — reads don't commit.
fn describe(command: &Command) -> &'static str {
    match command {
        Command::Type { action } => match action {
            TypeAction::Add { .. } => "add type",
            TypeAction::Rm => "remove type",
            TypeAction::Edit => "rename type",
            TypeAction::List => "list types",
        },
        Command::Mode { action } => match action {
            ModeAction::Add { .. } => "add mode",
            ModeAction::Rm => "remove mode",
            ModeAction::Edit => "rename mode",
            ModeAction::List => "list modes",
        },
        Command::Source { action } => match action {
            SourceAction::Add { .. } => "add source",
            SourceAction::Rm => "remove source",
            SourceAction::Edit => "rename source",
            SourceAction::List => "list sources",
        },
        Command::Session { action } => match action {
            SessionAction::Add => "add session",
            SessionAction::Rm => "remove session",
            SessionAction::Edit => "edit session",
            SessionAction::List => "list sessions",
        },
        Command::Log => "log session",
        _ => "update",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_labels_mutations() {
        assert_eq!(describe(&Command::Log), "log session");
        assert_eq!(
            describe(&Command::Session { action: SessionAction::Add }),
            "add session"
        );
        assert_eq!(
            describe(&Command::Source { action: SourceAction::Rm }),
            "remove source"
        );
        assert_eq!(
            describe(&Command::Type { action: TypeAction::Edit }),
            "rename type"
        );
    }
}
