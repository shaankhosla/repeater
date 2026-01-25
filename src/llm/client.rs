use crate::llm::secrets::API_KEY_ENV;
use crate::utils::ask_yn;
use anyhow::{Context, Result, anyhow, bail};

use async_openai::{Client, config::OpenAIConfig};

use super::secrets::{
    ApiKeySource, ProviderAuth, get_api_key_from_sources, prompt_for_llm_details, store_llm_details,
};

#[derive(Clone, Debug)]
pub struct LlmClient {
    pub client: Client<OpenAIConfig>,
    pub llm_auth: ProviderAuth,
}

pub async fn ensure_client(user_prompt: &str) -> Result<LlmClient> {
    let lookup = get_api_key_from_sources()?;
    let (llm_auth, prompted_for_key) = if let Some(llm_auth) = lookup.llm_auth {
        (llm_auth, false)
    } else {
        let llm_auth = get_auth_and_store(user_prompt).await?;
        (llm_auth, true)
    };

    // If we didn't prompt for the API key (it already existed), confirm with the user
    if !prompted_for_key {
        let ok = ask_yn(user_prompt.to_string());
        if !ok {
            bail!("LLM client not initialized.");
        }
    }

    let client = initialize_client(&llm_auth)?;
    Ok(LlmClient { client, llm_auth })
}

pub async fn get_auth_and_store(user_prompt: &str) -> Result<ProviderAuth> {
    let llm_auth = prompt_for_llm_details(user_prompt).await?;

    store_llm_details(&llm_auth)?;
    Ok(llm_auth)
}

pub async fn test_configured_api_key() -> Result<ApiKeySource> {
    let lookup = get_api_key_from_sources()?;
    let llm_auth = lookup.llm_auth.ok_or_else(|| {
        anyhow!(
            "LLM features are disabled. To enable, set {} or run `repeater llm key --set`.",
            API_KEY_ENV
        )
    })?;
    let source = lookup.source.ok_or_else(|| {
        anyhow!(
            "LLM features are disabled. To enable, set {} or run `repeater llm key --set`.",
            API_KEY_ENV
        )
    })?;
    let client = initialize_client(&llm_auth)?;
    get_models(&client).await?;
    Ok(source)
}

pub fn initialize_client(llm_auth: &ProviderAuth) -> Result<Client<OpenAIConfig>> {
    let mut config = OpenAIConfig::new();

    if let Some(key) = &llm_auth.key {
        config = config.with_api_key(key);
    }

    config = config.with_api_base(llm_auth.base_url.clone());

    let client = Client::with_config(config);
    Ok(client)
}

pub async fn get_models(client: &Client<OpenAIConfig>) -> Result<Vec<String>> {
    let models = client
        .models()
        .list()
        .await
        .context("Failed to fetch models from provider")?
        .data
        .iter()
        .map(|m| m.id.clone())
        .collect();

    Ok(models)
}
