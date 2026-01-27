use anyhow::{Context, Result, bail};
use async_openai::types::responses::{CreateResponseArgs, InputMessage, InputRole};

use super::LlmClient;
use serde_json::Value;

pub async fn request_single_text_response(
    client: &LlmClient,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    let request = CreateResponseArgs::default()
        .model(client.llm_auth.model.as_str())
        .max_output_tokens(5000_u32)
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
