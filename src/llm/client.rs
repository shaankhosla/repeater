use crate::llm::secrets::API_KEY_ENV;
use crate::utils::ask_yn;
use anyhow::{Context, Result, anyhow, bail};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};

use super::secrets::{
    ApiKeySource, ProviderAuth, get_api_key_from_sources, prompt_for_llm_details, store_llm_details,
};

#[derive(Clone, Debug)]
pub struct LlmClient {
    pub client: reqwest::Client,
    pub llm_auth: ProviderAuth,
}

fn build_headers(auth: &ProviderAuth) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    if let Some(key) = &auth.key {
        if auth.name == "Anthropic" {
            headers.insert(
                "x-api-key",
                HeaderValue::from_str(key).context("Invalid API key header value")?,
            );
            headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        } else {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", key))
                    .context("Invalid authorization header value")?,
            );
        }
    }

    Ok(headers)
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
    get_models(&llm_auth).await?;
    Ok(source)
}

pub fn initialize_client(llm_auth: &ProviderAuth) -> Result<reqwest::Client> {
    let headers = build_headers(llm_auth)?;
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .context("Failed to build HTTP client")?;
    Ok(client)
}

pub async fn get_models(auth: &ProviderAuth) -> Result<Vec<String>> {
    let base_url = auth.base_url.trim_end_matches('/');
    let url = format!("{}/models", base_url);

    let client = initialize_client(auth)?;

    let response = client
        .get(&url)
        .send()
        .await
        .context("Failed to fetch models from provider")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        bail!("Model list request failed ({}): {}", status, error_text);
    }

    let parsed: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse models response")?;

    let mut models = Vec::new();

    // Handle OpenAI format: { data: [ { id: "..." }, ... ] }
    if let Some(data) = parsed.get("data").and_then(|d| d.as_array()) {
        for entry in data {
            if let Some(id) = entry.get("id").and_then(|i| i.as_str()) {
                models.push(id.to_string());
            } else if let Some(name) = entry.get("name").and_then(|n| n.as_str()) {
                models.push(name.to_string());
            }
        }
    }
    // Handle plain array format: [ "model1", "model2", ... ]
    else if let Some(array) = parsed.as_array() {
        for entry in array {
            if let Some(model) = entry.as_str() {
                models.push(model.to_string());
            }
        }
    }

    Ok(models)
}
