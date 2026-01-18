use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use dialoguer::{Password, theme::ColorfulTheme};
use serde_json::{Value, json};

use crate::utils::get_data_dir;
use crate::{palette::Palette, utils::strip_controls_and_escapes};

pub const API_KEY_ENV: &str = "REPEATER_OPENAI_API_KEY";

const AUTH_FILE_NAME: &str = "auth.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeySource {
    Environment,
    AuthFile,
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

    let obj = auth
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Auth file root must be a JSON object"))?;

    if obj.remove("openai").is_none() {
        return Ok(false);
    }

    if obj.is_empty() {
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
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        bail!("Cannot store an empty API key");
    }

    let auth_path = auth_file_path()?;
    let mut auth = read_auth_file(&auth_path)?.unwrap_or_else(|| json!({}));
    let obj = auth
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Auth file root must be a JSON object"))?;

    obj.insert(
        "openai".to_string(),
        json!({
            "type": "api",
            "key": trimmed,
        }),
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
        .get("openai")
        .and_then(|entry| entry.get("key"))
        .and_then(|key| key.as_str())
        .map(str::trim)
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

fn read_auth_file(path: &PathBuf) -> Result<Option<Value>> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            if contents.trim().is_empty() {
                return Ok(Some(json!({})));
            }
            let parsed: Value = serde_json::from_str(&contents)
                .with_context(|| format!("Failed to parse auth file at {}", path.display()))?;
            Ok(Some(parsed))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => {
            Err(err).with_context(|| format!("Failed to read auth file at {}", path.display()))
        }
    }
}

fn write_auth_file(path: &PathBuf, value: &Value) -> Result<()> {
    let contents = serde_json::to_string_pretty(value)?;
    fs::write(path, format!("{}\n", contents))
        .with_context(|| format!("Failed to write auth file at {}", path.display()))?;
    Ok(())
}
