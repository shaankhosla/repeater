use std::env;

use crate::utils::strip_controls_and_escapes;
use anyhow::{Context, Result, anyhow, bail};

use rpassword::read_password;

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
    match entry.delete_password() {
        Ok(()) => Ok(true),
        Err(KeyringError::NoEntry) => Ok(false),
        Err(err) => Err(anyhow!(err)),
    }
}

pub fn prompt_for_api_key(prompt: &str) -> Result<String> {
    let dim = "\x1b[2m";
    let reset = "\x1b[0m";
    let green = "\x1b[32m";

    println!("\n{}", prompt);
    println!(
        "{green}Enter your OpenAI API key{reset} (https://platform.openai.com/account/api-keys) to enable the LLM helper. It's stored locally for future use.",
        green = green,
        reset = reset
    );
    println!(
        "{dim}This feature is optional, leave the field blank to skip.{reset}",
        dim = dim,
        reset = reset
    );

    let mut input = read_password().context("Failed to read API key")?;
    // Make input safe for use in a header
    input = strip_controls_and_escapes(&input);
    Ok(input.trim().to_string())
}

pub fn store_api_key(api_key: &str) -> Result<()> {
    let trimmed = api_key.trim();

    if trimmed.is_empty() {
        bail!("Cannot store an empty API key");
    }
    let entry = Entry::new(SERVICE, USERNAME)?;
    entry.set_password(trimmed)?;
    Ok(())
}

pub fn resolve_configured_api_key() -> Result<Option<(String, ApiKeySource)>> {
    if let Some(env_key) = load_env_api_key() {
        return Ok(Some((env_key, ApiKeySource::Environment)));
    }

    if let Some(stored) = load_stored_api_key()? {
        return Ok(Some((stored, ApiKeySource::Keyring)));
    }

    Ok(None)
}

fn load_env_api_key() -> Option<String> {
    match env::var(API_KEY_ENV) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => None,
    }
}

fn load_stored_api_key() -> Result<Option<String>> {
    let entry = Entry::new(SERVICE, USERNAME)?;
    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(err) => Err(anyhow!(err)),
    }
}
