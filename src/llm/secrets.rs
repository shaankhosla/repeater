use std::env;

use dialoguer::{Password, theme::ColorfulTheme};

use crate::{palette::Palette, utils::strip_controls_and_escapes};
use anyhow::{Result, bail};

use keyring::{Entry, Error as KeyringError};

pub const API_KEY_ENV: &str = "REPEATER_OPENAI_API_KEY";

const SERVICE: &str = "com.repeater";
const USERNAME: &str = "openai:default";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeySource {
    Environment,
    Keyring,
}

impl ApiKeySource {
    pub fn description(&self) -> &'static str {
        match self {
            ApiKeySource::Environment => "environment variable",
            ApiKeySource::Keyring => "local keyring",
        }
    }
}

pub fn clear_api_key() -> Result<bool> {
    let entry = Entry::new(SERVICE, USERNAME)?;
    match entry.delete_credential() {
        Ok(()) => Ok(true),
        Err(KeyringError::NoEntry) => Ok(false),
        Err(err) => bail!(err),
    }
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
    pub keyring_entry: Option<Entry>,
}

pub fn store_api_key(api_key: &str) -> Result<()> {
    let entry = Entry::new(SERVICE, USERNAME)?;
    store_api_key_with_entry(entry, api_key)
}

pub fn store_api_key_with_entry(entry: Entry, api_key: &str) -> Result<()> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        bail!("Cannot store an empty API key");
    }

    entry.set_password(trimmed)?;
    Ok(())
}

pub fn get_api_key_from_sources() -> Result<ApiKeyLookup> {
    // 1. Environment variable
    if let Ok(value) = env::var(API_KEY_ENV)
        && !value.trim().is_empty()
    {
        return Ok(ApiKeyLookup {
            api_key: Some(value),
            source: Some(ApiKeySource::Environment),
            keyring_entry: None,
        });
    }

    // 2. Keyring
    let entry = Entry::new(SERVICE, USERNAME)?;
    match entry.get_password() {
        Ok(password) => Ok(ApiKeyLookup {
            api_key: Some(password),
            source: Some(ApiKeySource::Keyring),
            keyring_entry: Some(entry),
        }),
        Err(KeyringError::NoEntry) => Ok(ApiKeyLookup {
            api_key: None,
            source: None,
            keyring_entry: Some(entry),
        }),
        Err(err) => bail!(err),
    }
}
