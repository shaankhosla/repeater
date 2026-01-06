use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueHint};

use repeater::crud::DB;
use repeater::{check, create, drill, import};

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
        } => {
            drill::run(&db, paths, card_limit, new_card_limit).await?;
        }
        Command::Check { paths } => {
            let _ = check::run(&db, paths).await?;
        }
        Command::Create { path } => {
            create::run(&db, path).await?;
        }
        Command::Import {
            anki_path,
            export_path,
        } => import::run(&db, &anki_path, &export_path).await.with_context(|| "Importing from Anki is a work in progress, please report issues on https://github.com/shaankhosla/repeater")?,
    }

    Ok(())
}
