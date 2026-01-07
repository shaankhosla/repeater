use std::env;

use anyhow::{Context, Result, anyhow, bail};
use async_openai::types::{
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs,
};
use async_openai::{Client, config::OpenAIConfig};
use rpassword::read_password;

const SERVICE: &str = "com.repeater";
const USERNAME: &str = "openai:default";

use keyring::{Entry, Error as KeyringError};

const CLOZE_MODEL: &str = "gpt-5-nano";
const SYSTEM_PROMPT: &str = r#"
You convert flashcards into Cloze deletions.
A Cloze deletion is denoted by square brackets: [hidden text].
Only add one Cloze deletion.
"#;

const USER_PROMPT_HEADER: &str = r#"
Turn the following text into a Cloze card by inserting [] around the hidden portion.
Return the exact same text as below, but just with the addition of brackets around the Cloze deletion. 
Your goal is to highlight the part of the flashcard you believe is most critical for a studying user to be able to recall.
It can be a word or a small phrase. For example, if you were shown the follwing text:

C: Speech is produced in Broca's area.

This might be a good response to produce:

C: Speech is produced in [Broca's] area.

This is the text you should generate the Cloze deletion for:

"#;

pub const API_KEY_ENV: &str = "REPEATER_OPENAI_API_KEY";

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

pub fn ensure_client(user_prompt: &str) -> Result<Client<OpenAIConfig>> {
    let key = match resolve_configured_api_key()? {
        Some((api_key, _source)) => api_key,
        None => {
            let api_key = prompt_for_api_key(user_prompt)?;
            if api_key.is_empty() {
                return Err(anyhow!(
                    "No API key provided. Set {} or run `repeat llm key --set <KEY>`.",
                    API_KEY_ENV
                ));
            }
            store_api_key(&api_key)?;
            api_key
        }
    };
    let client = initialize_client(&key)?;
    Ok(client)
}

pub async fn test_configured_api_key() -> Result<ApiKeySource> {
    let (key, source) = resolve_configured_api_key()?.ok_or_else(|| {
        anyhow!(
            "LLM features are disabled. To enable, set {} or run `repeat llm key --set <KEY>`.",
            API_KEY_ENV
        )
    })?;
    let client = initialize_client(&key)?;
    healthcheck_client(&client).await?;
    Ok(source)
}

pub fn clear_api_key() -> Result<bool> {
    let entry = Entry::new(SERVICE, USERNAME)?;
    match entry.delete_password() {
        Ok(()) => Ok(true),
        Err(KeyringError::NoEntry) => Ok(false),
        Err(err) => Err(anyhow!(err)),
    }
}

fn initialize_client(api_key: &str) -> Result<Client<OpenAIConfig>> {
    let config = OpenAIConfig::new().with_api_key(api_key);

    let client = Client::with_config(config);
    Ok(client)
}

async fn healthcheck_client(client: &Client<OpenAIConfig>) -> Result<()> {
    client
        .models()
        .list()
        .await
        .context("Failed to validate API key with OpenAI")?;
    Ok(())
}

pub async fn request_cloze(client: &Client<OpenAIConfig>, text: &str) -> Result<String> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(CLOZE_MODEL)
        .max_completion_tokens(5000_u32)
        .messages([
            ChatCompletionRequestSystemMessageArgs::default()
                .content(SYSTEM_PROMPT)
                .build()?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(build_user_prompt(text))
                .build()?
                .into(),
        ])
        .build()?;

    let response = client.chat().create(request).await?;

    let output = response
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .ok_or_else(|| anyhow!("No content returned from model"))?;

    Ok(output)
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

    let input = read_password().context("Failed to read API key")?;
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

fn resolve_configured_api_key() -> Result<Option<(String, ApiKeySource)>> {
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

fn build_user_prompt(text: &str) -> String {
    format!("{USER_PROMPT_HEADER}{text}")
}
