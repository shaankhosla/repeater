use std::collections::HashMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use dialoguer::{Password, theme::ColorfulTheme};
use serde::{Deserialize, Serialize};

use crate::utils::get_data_dir;
use crate::utils::trim_line;
use crate::{palette::Palette, utils::strip_controls_and_escapes};

pub const API_KEY_ENV: &str = "REPEATER_OPENAI_API_KEY";

const AUTH_FILE_NAME: &str = "auth.json";
const OPENAI_PROVIDER: &str = "openai";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeySource {
    Environment,
    AuthFile,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AuthFile {
    #[serde(flatten)]
    providers: HashMap<String, ProviderAuth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderAuth {
    key: String,
}

impl ApiKeySource {
    pub fn description(&self) -> &'static str {
        match self {
            ApiKeySource::Environment => "environment variable",
            ApiKeySource::AuthFile => "local auth file",
        }
    }
}

pub fn clear_api_key() -> Result<bool> {
    let auth_path = auth_file_path()?;
    let Some(mut auth) = read_auth_file(&auth_path)? else {
        return Ok(false);
    };

    if auth.providers.remove(OPENAI_PROVIDER).is_none() {
        return Ok(false);
    }

    if auth.providers.is_empty() {
        fs::remove_file(&auth_path).with_context(|| {
            format!(
                "Failed to remove empty auth file at {}",
                auth_path.display()
            )
        })?;
        return Ok(true);
    }

    write_auth_file(&auth_path, &auth)?;
    Ok(true)
}

pub fn prompt_for_api_key(prompt: &str) -> Result<String> {
    println!("\n{}", prompt);
    println!(
        "{} (https://platform.openai.com/account/api-keys) to enable the LLM helper. It's stored locally for future use.",
        Palette::paint(Palette::SUCCESS, "Enter your OpenAI API key")
    );
    println!(
        "{}",
        Palette::dim("This feature is optional, leave the field blank to skip.")
    );
    let raw_password = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("API Key")
        .allow_empty_password(true)
        .interact()
        .unwrap();

    let password = strip_controls_and_escapes(&raw_password);
    Ok(password.trim().to_string())
}

#[derive(Debug)]
pub struct ApiKeyLookup {
    pub api_key: Option<String>,
    pub source: Option<ApiKeySource>,
}

pub fn store_api_key(api_key: &str) -> Result<()> {
    let trimmed = trim_line(api_key).with_context(|| "Cannot store an empty API key")?;

    let auth_path = auth_file_path()?;
    let mut auth = read_auth_file(&auth_path)?.unwrap_or_default();

    auth.providers.insert(
        OPENAI_PROVIDER.to_string(),
        ProviderAuth {
            key: trimmed.to_string(),
        },
    );

    write_auth_file(&auth_path, &auth)
}

pub fn get_api_key_from_sources() -> Result<ApiKeyLookup> {
    // 1. Environment variable
    if let Ok(value) = env::var(API_KEY_ENV)
        && !value.trim().is_empty()
    {
        return Ok(ApiKeyLookup {
            api_key: Some(value),
            source: Some(ApiKeySource::Environment),
        });
    }

    // 2. Auth file
    let auth_path = auth_file_path()?;
    let Some(auth) = read_auth_file(&auth_path)? else {
        return Ok(ApiKeyLookup {
            api_key: None,
            source: None,
        });
    };

    let key = auth
        .providers
        .get(OPENAI_PROVIDER)
        .map(|entry| entry.key.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    if let Some(api_key) = key {
        return Ok(ApiKeyLookup {
            api_key: Some(api_key),
            source: Some(ApiKeySource::AuthFile),
        });
    }

    Ok(ApiKeyLookup {
        api_key: None,
        source: None,
    })
}

fn auth_file_path() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    Ok(data_dir.join(AUTH_FILE_NAME))
}

fn read_auth_file(path: &Path) -> Result<Option<AuthFile>> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(parse_auth_contents(&contents, path)?),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => {
            Err(err).with_context(|| format!("Failed to read auth file at {}", path.display()))
        }
    }
}

fn write_auth_file(path: &Path, value: &AuthFile) -> Result<()> {
    let contents = serialize_auth(value)?;
    fs::write(path, contents)
        .with_context(|| format!("Failed to write auth file at {}", path.display()))?;
    Ok(())
}

fn parse_auth_contents(contents: &str, path: &Path) -> Result<Option<AuthFile>> {
    if contents.trim().is_empty() {
        return Ok(Some(AuthFile::default()));
    }

    let parsed: AuthFile = serde_json::from_str(contents)
        .with_context(|| format!("Failed to parse auth file at {}", path.display()))?;
    Ok(Some(parsed))
}

fn serialize_auth(value: &AuthFile) -> Result<String> {
    let contents = serde_json::to_string_pretty(value)?;
    Ok(format!("{}\n", contents))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parse_auth_contents_handles_empty() {
        let path = Path::new("auth.json");
        let parsed = parse_auth_contents("   \n", path).unwrap();
        let auth = parsed.expect("expected auth file for empty contents");
        assert!(auth.providers.is_empty());
    }

    #[test]
    fn serialize_auth_adds_trailing_newline() {
        let mut auth = AuthFile::default();
        auth.providers.insert(
            OPENAI_PROVIDER.to_string(),
            ProviderAuth {
                key: "test-key".to_string(),
            },
        );

        let serialized = serialize_auth(&auth).unwrap();
        assert!(serialized.ends_with('\n'));
        let parsed: AuthFile = serde_json::from_str(serialized.trim()).unwrap();
        assert_eq!(
            parsed
                .providers
                .get(OPENAI_PROVIDER)
                .map(|entry| entry.key.as_str()),
            Some("test-key")
        );
    }
    #[test]
    fn file_doesnt_exist() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let unexisting_file_auth = read_auth_file(&path).unwrap();
        assert!(unexisting_file_auth.is_none());
    }

    #[test]
    fn overwrite() {
        let dir = tempdir().unwrap();
        let auth_path = dir.path().join("auth.json");

        let mut auth = AuthFile::default();
        auth.providers.insert(
            OPENAI_PROVIDER.to_string(),
            ProviderAuth {
                key: "fake-key".to_string(),
            },
        );
        write_auth_file(&auth_path, &auth).unwrap();

        let mut auth = AuthFile::default();
        auth.providers.insert(
            OPENAI_PROVIDER.to_string(),
            ProviderAuth {
                key: "actual-key".to_string(),
            },
        );

        write_auth_file(&auth_path, &auth).unwrap();
        let read_back = read_auth_file(&auth_path).unwrap();
        let auth = read_back.expect("expected auth file to exist");
        assert_eq!(
            auth.providers
                .get(OPENAI_PROVIDER)
                .map(|entry| entry.key.as_str()),
            Some("actual-key")
        );
    }

    #[test]
    fn write_and_read_auth_file_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let mut auth = AuthFile::default();
        auth.providers.insert(
            OPENAI_PROVIDER.to_string(),
            ProviderAuth {
                key: "saved-key".to_string(),
            },
        );

        write_auth_file(&path, &auth).unwrap();
        let read_back = read_auth_file(&path).unwrap();
        let auth = read_back.expect("expected auth file to exist");
        assert_eq!(
            auth.providers
                .get(OPENAI_PROVIDER)
                .map(|entry| entry.key.as_str()),
            Some("saved-key")
        );
    }
}
