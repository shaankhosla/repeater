use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueHint};

use repeater::crud::DB;
use repeater::llm::client;
use repeater::{
    commands::{
        check, create,
        drill::{self, DrillOptions},
        drill_session::{self, DrillSessionError, StartOptions},
    },
    fsrs::ReviewStatus,
};
use repeater::{import, llm, palette::Palette};

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
    Drill(DrillArgs),
    /// Work through due cards using individual CLI commands
    DrillSession {
        #[command(subcommand)]
        command: DrillSessionCommand,
    },
    /// Re-index decks and show collection stats
    Check(CheckArgs),
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
}

#[derive(Args, Debug)]
struct CardSourceArgs {
    /// Paths to cards or directories containing them.
    /// You can pass a single file, multiple files, or a directory.
    #[arg(
        value_name = "PATHS",
        num_args = 0..,
        default_value = ".",
        value_hint = ValueHint::AnyPath
    )]
    paths: Vec<PathBuf>,
    /// Read cards from Apple Notes instead of local files (macOS only).
    #[arg(long, default_value_t = false, conflicts_with = "paths")]
    apple_notes: bool,
}

#[derive(Args, Debug)]
struct DrillSettingsArgs {
    /// Rephrase questions via the LLM helper before presenting them.
    #[arg(long = "rephrase", default_value_t = false)]
    rephrase_questions: bool,
    /// Goal retention FSRS should use when a card is marked.
    #[arg(long, default_value_t = 0.9)]
    retention: f32,
}

#[derive(Args, Debug)]
struct DrillArgs {
    #[command(flatten)]
    source: CardSourceArgs,
    /// Maximum number of cards to drill. By default, all cards due today are drilled.
    #[arg(long, value_name = "COUNT")]
    card_limit: Option<usize>,
    /// Maximum number of new cards to drill.
    #[arg(long, value_name = "COUNT")]
    new_card_limit: Option<usize>,
    #[command(flatten)]
    settings: DrillSettingsArgs,
    /// Randomize the order of cards in the drill session.
    #[arg(long, default_value_t = false)]
    shuffle: bool,
}

impl From<DrillArgs> for DrillOptions {
    fn from(args: DrillArgs) -> Self {
        Self {
            paths: args.source.paths,
            card_limit: args.card_limit,
            new_card_limit: args.new_card_limit,
            rephrase_questions: args.settings.rephrase_questions,
            shuffle: args.shuffle,
            retention: args.settings.retention,
            apple_notes: args.source.apple_notes,
        }
    }
}

#[derive(Args, Debug)]
struct DrillSessionStartArgs {
    #[command(flatten)]
    source: CardSourceArgs,
    /// Randomly select from the cards currently due.
    #[arg(long, default_value_t = false)]
    shuffle: bool,
    #[command(flatten)]
    settings: DrillSettingsArgs,
}

#[derive(Args, Debug)]
struct CheckArgs {
    #[command(flatten)]
    source: CardSourceArgs,
    /// Print a plain summary instead of the TUI dashboard.
    #[arg(long, default_value_t = false)]
    plain: bool,
}

#[derive(Subcommand, Debug)]
enum DrillSessionCommand {
    /// Create a durable agent-driven drill session
    Start(DrillSessionStartArgs),
    /// Show the next card due for review
    Next {
        /// Token identifying the drill session
        #[arg(value_name = "SESSION_ID")]
        session_id: String,
    },
    /// Reveal the answer to a pending review
    Reveal {
        /// Token identifying the review
        #[arg(value_name = "REVIEW_ID")]
        review_id: String,
    },
    /// Mark a revealed review as pass or fail
    Mark {
        /// Token identifying the review
        #[arg(value_name = "REVIEW_ID")]
        review_id: String,
        /// Review result to record
        #[arg(value_name = "RESULT")]
        result: ReviewStatus,
    },
}

#[tokio::main]
async fn main() {
    if let Err(err) = run_cli().await {
        if let Some(session_error) = err.downcast_ref::<DrillSessionError>() {
            eprintln!("{}", session_error.json());
        } else {
            eprintln!("{err:?}");
        }
        std::process::exit(1);
    }
}

async fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    let db = DB::new().await?;

    match cli.command {
        Command::Drill(args) => {
            drill::run(&db, args.into()).await?;
        }
        Command::DrillSession { command } => match command {
            DrillSessionCommand::Start(args) => {
                drill_session::start(
                    &db,
                    StartOptions {
                        paths: args.source.paths,
                        apple_notes: args.source.apple_notes,
                        retention: args.settings.retention,
                        rephrase_questions: args.settings.rephrase_questions,
                        shuffle: args.shuffle,
                    },
                )
                .await?;
            }
            DrillSessionCommand::Next { session_id } => {
                drill_session::next(&db, &session_id).await?;
            }
            DrillSessionCommand::Reveal { review_id } => {
                drill_session::reveal(&db, &review_id).await?;
            }
            DrillSessionCommand::Mark { review_id, result } => {
                drill_session::mark(&db, &review_id, result).await?;
            }
        },
        Command::Check(args) => {
            let _ = check::run(
                &db,
                args.source.paths,
                args.plain,
                args.source.apple_notes,
            )
            .await?;
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
