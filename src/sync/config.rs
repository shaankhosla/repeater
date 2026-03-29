use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::utils::get_data_dir;

const SYNC_FILE_NAME: &str = "sync.json";
const DEFAULT_SYNC_ADDRESS: &str = "https://sync.repeater.dev";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub address: String,
    pub session_token: Option<String>,
    pub username: Option<String>,
    pub last_server_version: i64,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            address: DEFAULT_SYNC_ADDRESS.to_string(),
            session_token: None,
            username: None,
            last_server_version: 0,
        }
    }
}

impl SyncConfig {
    pub fn is_logged_in(&self) -> bool {
        self.session_token.is_some()
    }

    pub fn load() -> Result<Self> {
        let path = sync_file_path()?;
        read_sync_file(&path)
    }

    pub fn save(&self) -> Result<()> {
        let path = sync_file_path()?;
        write_sync_file(&path, self)
    }

    pub fn clear_session(&mut self) -> Result<()> {
        self.session_token = None;
        self.username = None;
        self.save()
    }
}

fn sync_file_path() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    Ok(data_dir.join(SYNC_FILE_NAME))
}

fn read_sync_file(path: &Path) -> Result<SyncConfig> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            if contents.trim().is_empty() {
                return Ok(SyncConfig::default());
            }
            let config: SyncConfig = serde_json::from_str(&contents)
                .with_context(|| format!("Failed to parse sync config at {}", path.display()))?;
            Ok(config)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(SyncConfig::default()),
        Err(err) => {
            Err(err).with_context(|| format!("Failed to read sync config at {}", path.display()))
        }
    }
}

fn write_sync_file(path: &Path, config: &SyncConfig) -> Result<()> {
    let contents = serde_json::to_string_pretty(config)?;
    fs::write(path, format!("{}\n", contents))
        .with_context(|| format!("Failed to write sync config at {}", path.display()))?;
    Ok(())
}
