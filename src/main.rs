use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueHint};

use repeater::commands::{check, create, drill};
use repeater::crud::DB;
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
        #[arg(long, value_name = "KEY", conflicts_with = "clear")]
        set: Option<String>,
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
        } => {
            drill::run(&db, paths, card_limit, new_card_limit, rephrase_questions).await?;
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

async fn handle_llm_command(set: Option<String>, clear: bool, test: bool) -> Result<()> {
    let mut action_taken = false;

    if let Some(key) = set {
        llm::store_api_key(&key)?;
        println!("Stored OpenAI API key in the local auth file.");
        action_taken = true;
    }

    if clear {
        let removed = llm::clear_api_key()?;
        if removed {
            println!("Removed the stored OpenAI API key.");
        } else {
            println!("No OpenAI API key found in the auth file.");
        }
        action_taken = true;
    }

    if test {
        let source = llm::test_configured_api_key().await?;
        println!("OpenAI API key from the {} is valid.", source.description());
        action_taken = true;
    }

    if !action_taken {
        bail!("No action provided. Use --set, --clear, or --test.");
    }
    Ok(())
}
