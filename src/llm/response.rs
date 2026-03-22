use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use super::LlmClient;

const MAX_TOKENS: u32 = 5000;

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<ChatMessage>,
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
    chat_completion(client, system_prompt, user_prompt).await
}

async fn chat_completion(
    client: &LlmClient,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    let base_url = client.llm_auth.base_url.trim_end_matches('/');
    let url = format!("{}/chat/completions", base_url);

    let request_body = ChatCompletionRequest {
        model: client.llm_auth.model.clone(),
        max_tokens: MAX_TOKENS,
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_prompt.to_string(),
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

    if let Some(content) = completion
        .choices
        .first()
        .and_then(|c| c.message.content.as_ref())
    {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    bail!("Empty response from model")
}
