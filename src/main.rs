use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueHint};

use repeater::commands::{check, create, drill};
use repeater::crud::DB;
use repeater::llm::client;
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
        /// Randomize the order of cards in the drill session
        #[arg(long, default_value_t = false)]
        shuffle: bool,
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
        } => {
            drill::run(&db, paths, card_limit, new_card_limit, rephrase_questions, shuffle).await?;
        }
        Command::Check { paths, plain } => {
            let _ = check::run(&db, paths, plain).await?;
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
            Palette::paint(Palette::SUCCESS, "Stored the LLM config in the local auth file.")
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
            false => println!(
                "{}",
                Palette::dim("The stored LLM config did not exist.")
            ),
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
