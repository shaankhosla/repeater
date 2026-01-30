use anyhow::{Context, Result, bail};
use async_openai::types::{
    chat::{
        ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessage,
        CreateChatCompletionRequestArgs,
    },
    responses::{CreateResponseArgs, InputMessage, InputRole},
};

use super::LlmClient;
use serde_json::Value;

const MAX_TOKENS: u32 = 5000;

pub async fn request_single_text_response(
    client: &LlmClient,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    match chat_response(client, system_prompt, user_prompt).await {
        Ok(resp) => Ok(resp),
        Err(primary_err) => chat_completion(client, system_prompt, user_prompt)
            .await
            .with_context(|| {
                format!("Fallback chat_completion failed after responses API error: {primary_err}")
            }),
    }
}

async fn chat_response(
    client: &LlmClient,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    let request = CreateResponseArgs::default()
        .model(client.llm_auth.model.as_str())
        .max_output_tokens(MAX_TOKENS)
        .input(vec![
            InputMessage {
                role: InputRole::System,
                content: vec![system_prompt.into()],
                status: None,
            },
            InputMessage {
                role: InputRole::User,
                content: vec![user_prompt.into()],
                status: None,
            },
        ])
        .build()?;

    let response: Value = client
        .client
        .responses()
        .create_byot(request)
        .await
        .with_context(|| "Failed to get response from LLM")?;

    if let Some(content) = response["output"][0]["content"][0]["text"].as_str() {
        let trimmed_content = content.trim();
        if !trimmed_content.is_empty() {
            return Ok(trimmed_content.to_string());
        }
    }

    bail!(format!("Invalid response from model:\n{response}"))
}
async fn chat_completion(
    client: &LlmClient,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    let request = CreateChatCompletionRequestArgs::default()
        .max_tokens(MAX_TOKENS)
        .model(client.llm_auth.model.as_str())
        .messages([
            ChatCompletionRequestSystemMessage::from(system_prompt).into(),
            ChatCompletionRequestUserMessage::from(user_prompt).into(),
        ])
        .build()?;

    let response: Value = client
        .client
        .chat()
        .create_byot(request)
        .await
        .with_context(|| "Failed to get response from LLM")?;

    if let Some(content) = response["choices"][0]["message"]["content"].as_str() {
        let trimmed_content = content.trim();
        if !trimmed_content.is_empty() {
            return Ok(trimmed_content.to_string());
        }
    }
    bail!(format!("Invalid response from model:\n{response}"))
}
