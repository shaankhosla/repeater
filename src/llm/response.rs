use anyhow::{Result, bail};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::responses::{
        CreateResponseArgs, InputMessage, InputRole, OutputItem, OutputMessageContent,
    },
};

pub async fn request_single_text_response(
    client: &Client<OpenAIConfig>,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    let request = CreateResponseArgs::default()
        .model(model)
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

    let response = client.responses().create(request).await?;

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
