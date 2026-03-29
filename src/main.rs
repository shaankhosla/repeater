use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueHint};

use repeater::commands::{
    check, create,
    drill::{self, DrillOptions},
};
use repeater::crud::DB;
use repeater::llm::client;
use repeater::palette::Palette;
use repeater::sync::client::SyncClient;
use repeater::sync::config::SyncConfig;
use repeater::{import, llm};

#[derive(Parser, Debug)]
#[command(
    name = "repeater",
    version,
    about = "Spaced repetition for the terminal.",
    long_about = None,
    propagate_version = true,
    arg_required_else_help = true,
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Drill cards
    Drill {
        /// Paths to cards or directories containing them.
        /// You can pass a single file, multiple files, or a directory.
        #[arg(
            value_name = "PATHS",
            num_args = 0..,
            default_value = ".",
            value_hint = ValueHint::AnyPath
        )]
        paths: Vec<PathBuf>,
        /// Maximum number of cards to drill in a session. By default, all cards due today are drilled.
        #[arg(long, value_name = "COUNT")]
        card_limit: Option<usize>,
        /// Maximum number of new cards to drill in a session.
        #[arg(long, value_name = "COUNT")]
        new_card_limit: Option<usize>,
        /// Rephrase  card questions via the LLM helper before the session starts.
        #[arg(long = "rephrase", default_value_t = false)]
        rephrase_questions: bool,
        /// Randomize the order of cards in the drill session.
        #[arg(long, default_value_t = true)]
        shuffle: bool,
        /// Goal retention FSRS should use, this is your target probability of recalling a card at review time.
        #[arg(long, default_value_t = 0.9)]
        retention: f32,
        /// Drill cards from Apple Notes instead of local files (macOS only).
        #[arg(long, default_value_t = false, conflicts_with = "paths")]
        apple_notes: bool,
        /// Skip automatic sync before and after the drill session.
        #[arg(long, default_value_t = false)]
        no_sync: bool,
    },
    /// Re-index decks and show collection stats
    Check {
        #[arg(
            value_name = "PATHS",
            num_args = 0..,
            default_value = ".",
            value_hint = ValueHint::AnyPath
        )]
        paths: Vec<PathBuf>,
        /// Print a plain summary instead of the TUI dashboard
        #[arg(long, default_value_t = false)]
        plain: bool,
        /// Check cards from Apple Notes instead of local files (macOS only).
        #[arg(long, default_value_t = false, conflicts_with = "paths")]
        apple_notes: bool,
        /// Skip automatic sync before checking.
        #[arg(long, default_value_t = false)]
        no_sync: bool,
    },
    /// Create or append to a card
    Create {
        /// Card path
        #[arg(value_name = "PATH", value_hint = ValueHint::FilePath)]
        path: PathBuf,
    },
    /// Import from Anki
    Import {
        /// Anki export path. Must be an apkg file
        #[arg(value_name = "PATH", value_hint = ValueHint::FilePath)]
        anki_path: PathBuf,
        /// Directory to export to
        #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
        export_path: PathBuf,
    },
    /// Manage LLM helper settings
    Llm {
        /// Store a new API key in the local auth file
        #[arg(long, conflicts_with = "clear")]
        set: bool,
        /// Remove the stored API key from the local auth file
        #[arg(long, conflicts_with = "test")]
        clear: bool,
        /// Verify the configured API key by calling the OpenAI API
        #[arg(long, conflicts_with = "clear")]
        test: bool,
    },
    /// Sync review history across devices
    Sync {
        #[command(subcommand)]
        action: SyncAction,
    },
}

#[derive(Subcommand, Debug)]
enum SyncAction {
    /// Register a new account on the sync server
    Register,
    /// Login to an existing account
    Login,
    /// Logout and clear local session
    Logout,
    /// Show sync status
    Status,
    /// Manually push and pull changes
    Now,
    /// Start the sync server (requires --features server)
    #[cfg(feature = "server")]
    Server,
}

#[tokio::main]
async fn main() {
    if let Err(err) = run_cli().await {
        eprintln!("{:?}", err);
        std::process::exit(1);
    }
}

async fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    let db = DB::new().await?;

    match cli.command {
        Command::Drill {
            paths,
            card_limit,
            new_card_limit,
            rephrase_questions,
            shuffle,
            retention,
            apple_notes,
            no_sync,
        } => {
            if !no_sync {
                repeater::sync::sync(&db, true).await.ok();
            }
            drill::run(&db, DrillOptions {
                paths,
                card_limit,
                new_card_limit,
                rephrase_questions,
                shuffle,
                retention,
                apple_notes,
            }).await?;
            if !no_sync {
                repeater::sync::sync(&db, true).await.ok();
            }
        }
        Command::Check { paths, plain, apple_notes, no_sync } => {
            if !no_sync {
                repeater::sync::sync(&db, true).await.ok();
            }
            let _ = check::run(&db, paths, plain, apple_notes).await?;
        }
        Command::Create { path } => {
            create::run(&db, path).await?;
        }
        Command::Import {
            anki_path,
            export_path,
        } => {
            import::run(&db, &anki_path, &export_path)
                .await.with_context(|| "Importing from Anki is a work in progress, please report issues on https://github.com/shaankhosla/repeater")?
        },
        Command::Llm { set, clear, test } => handle_llm_command(set, clear, test).await?,
        Command::Sync { action } => handle_sync_command(&db, action).await?,
    }

    Ok(())
}

async fn handle_sync_command(db: &DB, action: SyncAction) -> Result<()> {
    match action {
        SyncAction::Register => {
            let mut config = SyncConfig::load()?;

            let address: String =
                dialoguer::Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
                    .with_prompt("Sync server address")
                    .default(config.address.clone())
                    .interact_text()?;
            config.address = address;

            let client = SyncClient::new(config.clone())?;

            let username: String =
                dialoguer::Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
                    .with_prompt("Username")
                    .interact_text()?;

            let password: String =
                dialoguer::Password::with_theme(&dialoguer::theme::ColorfulTheme::default())
                    .with_prompt("Password (min 8 characters)")
                    .with_confirmation("Confirm password", "Passwords don't match")
                    .interact()?;

            let resp = client.register(&username, &password).await?;
            config.session_token = Some(resp.token);
            config.username = Some(username.clone());
            config.save()?;

            println!(
                "{}",
                Palette::paint(
                    Palette::SUCCESS,
                    format!("Registered as '{}'. Sync is now active.", username)
                )
            );
        }
        SyncAction::Login => {
            let mut config = SyncConfig::load()?;

            let address: String =
                dialoguer::Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
                    .with_prompt("Sync server address")
                    .default(config.address.clone())
                    .interact_text()?;
            config.address = address;

            let client = SyncClient::new(config.clone())?;

            let username: String =
                dialoguer::Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
                    .with_prompt("Username")
                    .interact_text()?;

            let password: String =
                dialoguer::Password::with_theme(&dialoguer::theme::ColorfulTheme::default())
                    .with_prompt("Password")
                    .interact()?;

            let resp = client.login(&username, &password).await?;
            config.session_token = Some(resp.token);
            config.username = Some(username.clone());
            config.save()?;

            println!(
                "{}",
                Palette::paint(
                    Palette::SUCCESS,
                    format!("Logged in as '{}'. Sync is now active.", username)
                )
            );
        }
        SyncAction::Logout => {
            let mut config = SyncConfig::load()?;
            let username = config.username.clone().unwrap_or_default();
            config.clear_session()?;
            println!(
                "{}",
                Palette::paint(
                    Palette::SUCCESS,
                    format!(
                        "Logged out{}.",
                        if username.is_empty() {
                            String::new()
                        } else {
                            format!(" (was '{}')", username)
                        }
                    )
                )
            );
        }
        SyncAction::Status => {
            let config = SyncConfig::load()?;
            println!("Sync server: {}", config.address);
            if let Some(username) = &config.username {
                println!("Logged in as: {}", username);
                println!("Last synced version: {}", config.last_server_version);

                let client = SyncClient::new(config)?;
                match client.status().await {
                    Ok(status) => {
                        println!("Remote cards: {}", status.card_count);
                        println!("Remote version: {}", status.latest_version);
                    }
                    Err(e) => {
                        eprintln!("{}", Palette::dim(format!("Could not reach server: {}", e)));
                    }
                }
            } else {
                println!("Not logged in. Run `repeater sync register` or `repeater sync login`.");
            }
        }
        SyncAction::Now => {
            let synced = repeater::sync::sync(db, false).await?;
            if synced {
                println!("{}", Palette::paint(Palette::SUCCESS, "Sync complete."));
            } else {
                println!("Not logged in. Run `repeater sync register` or `repeater sync login`.");
            }
        }
        #[cfg(feature = "server")]
        SyncAction::Server => {
            repeater::sync::server::start_server().await?;
        }
    }
    Ok(())
}

async fn handle_llm_command(set: bool, clear: bool, test: bool) -> Result<()> {
    let mut action_taken = false;

    if set {
        let user_prompt = "Enter your OpenAI API key:";
        let _ = client::get_auth_and_store(user_prompt).await?;
        println!(
            "{}",
            Palette::paint(
                Palette::SUCCESS,
                "Stored the LLM config in the local auth file."
            )
        );
        action_taken = true;
    }

    if clear {
        action_taken = true;

        match llm::clear_api_key()? {
            true => println!(
                "{}",
                Palette::paint(Palette::SUCCESS, "Removed the stored LLM config.")
            ),
            false => println!("{}", Palette::dim("The stored LLM config did not exist.")),
        }
    }

    if test {
        let source = llm::test_configured_api_key().await?;
        println!(
            "{} {} {}",
            Palette::dim("LLM config from the"),
            Palette::paint(Palette::INFO, source.description()),
            Palette::paint(Palette::SUCCESS, "is valid.")
        );
        action_taken = true;
    }

    if !action_taken {
        bail!("No action provided. Use --set, --clear, or --test.");
    }
    Ok(())
}
