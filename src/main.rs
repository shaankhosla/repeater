use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueHint};

use repeat::crud::DB;
use repeat::{check, create, drill, import};

#[derive(Parser, Debug)]
#[command(
    name = "repeat",
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
    Import {
        /// Anki export path
        anki_path: String,
        /// Where to export to
        export_path: String,
    },
}

#[tokio::main]
async fn main() {
    if let Err(error) = run_cli().await {
        eprintln!("error: {error}");
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
        Args::Import {
            anki_path,
            export_path,
        } => {
            let anki_path = PathBuf::from(anki_path);
            let export_path = PathBuf::from(export_path);
            if let Err(err) = import::run(&db, &anki_path, &export_path).await {
                eprintln!("error: {err}");
            }
        }
    }

    Ok(())
}
