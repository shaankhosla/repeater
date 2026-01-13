use crate::llm::secrets::API_KEY_ENV;
use anyhow::{Context, Result, anyhow, bail};
use async_openai::{Client, config::OpenAIConfig};

use super::secrets::{ApiKeySource, get_api_key_from_sources, prompt_for_api_key, store_api_key};

pub fn ensure_client(user_prompt: &str) -> Result<Client<OpenAIConfig>> {
    let key = match get_api_key_from_sources()? {
        Some((api_key, _source)) => api_key,
        None => {
            let api_key = prompt_for_api_key(user_prompt)?;
            if api_key.is_empty() {
                bail!(
                    "No API key provided. Set {} or run `repeater llm key --set <KEY>`.",
                    API_KEY_ENV
                );
            }
            store_api_key(&api_key)?;
            api_key
        }
    };
    let client = initialize_client(&key)?;
    Ok(client)
}

pub async fn test_configured_api_key() -> Result<ApiKeySource> {
    let (key, source) = get_api_key_from_sources()?.ok_or_else(|| {
        anyhow!(
            "LLM features are disabled. To enable, set {} or run `repeater llm key --set <KEY>`.",
            API_KEY_ENV
        )
    })?;
    let client = initialize_client(&key)?;
    healthcheck_client(&client).await?;
    Ok(source)
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
