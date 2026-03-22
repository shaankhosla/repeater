use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use super::LlmClient;

const MAX_TOKENS: u32 = 5000;

#[derive(Debug, Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    content: Option<String>,
}

pub async fn request_single_text_response(
    client: &LlmClient,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    let base_url = client.llm_auth.base_url.trim_end_matches('/');
    let url = format!("{}/chat/completions", base_url);

    let request_body = ChatCompletionRequest {
        model: &client.llm_auth.model,
        max_tokens: MAX_TOKENS,
        messages: vec![
            ChatMessage {
                role: "system",
                content: system_prompt,
            },
            ChatMessage {
                role: "user",
                content: user_prompt,
            },
        ],
    };

    let response = client
        .client
        .post(&url)
        .json(&request_body)
        .send()
        .await
        .context("Failed to send chat completion request")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        bail!("Chat completion failed ({}): {}", status, error_text);
    }

    let completion: ChatCompletionResponse = response
        .json()
        .await
        .context("Failed to parse chat completion response")?;

    completion
        .choices
        .first()
        .and_then(|c| c.message.content.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("Empty response from model"))
}
