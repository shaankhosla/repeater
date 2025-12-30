use clap::Parser;
use std::path::PathBuf;

use repeat::crud::DB;
use repeat::{check, create, drill, import};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
enum Args {
    /// Drill cards
    Drill {
        /// Paths to cards or directories containing them.
        /// You can pass a single file, multiple files, or a directory.
        #[arg(value_name = "PATHS", num_args = 0.., default_value = ".")]
        paths: Vec<String>,
        /// Maximum number of cards to drill in a session. By default, all cards due today are drilled.
        #[arg(long)]
        card_limit: Option<usize>,
        /// Maximum number of new cards to drill in a session.
        #[arg(long)]
        new_card_limit: Option<usize>,
    },
    Check {
        #[arg(value_name = "PATHS", num_args = 0.., default_value = ".")]
        paths: Vec<String>,
    },
    /// Create or append to a card
    Create {
        /// Card path
        path: String,
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
    let args = Args::parse();
    let db = DB::new()
        .await
        .expect("Failed to connect to or initialize database");
    match args {
        Args::Drill {
            paths,
            card_limit,
            new_card_limit,
        } => {
            let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
            if let Err(error) = drill::run(&db, paths, card_limit, new_card_limit).await {
                eprintln!("error: {error}")
            }
        }
        Args::Check { paths } => {
            let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
            if let Err(error) = check::run(&db, paths).await {
                eprintln!("error: {error}")
            }
        }
        Args::Create { path } => {
            let card_path = PathBuf::from(path);
            if let Err(err) = create::run(&db, card_path).await {
                eprintln!("error: {err}");
            }
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
}
