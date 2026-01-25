use anyhow::{Context, Result, bail};
use async_openai::types::responses::{
    CreateResponseArgs, InputMessage, InputRole, OutputItem, OutputMessageContent,
};

use super::LlmClient;

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

    let response = client
        .client
        .responses()
        .create(request)
        .await
        .with_context(|| "Failed to get response from LLM")?;

    for item in response.output {
        if let OutputItem::Message(message) = item {
            for content in message.content {
                if let OutputMessageContent::OutputText(text) = content {
                    let trimmed = text.text.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    return Ok(trimmed.to_string());
                }
            }
        }
    }

    bail!("No text output returned from model")
}
