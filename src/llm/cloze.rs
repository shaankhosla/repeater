use anyhow::{Result, bail};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::responses::{
        CreateResponseArgs, InputMessage, InputRole, OutputItem, OutputMessageContent,
    },
};

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

pub async fn request_cloze(client: &Client<OpenAIConfig>, text: &str) -> Result<String> {
    let user_prompt = format!("{USER_PROMPT_HEADER}{text}");

    let request = CreateResponseArgs::default()
        .model(CLOZE_MODEL)
        .max_output_tokens(5000_u32)
        .input(vec![
            InputMessage {
                role: InputRole::System,
                content: vec![SYSTEM_PROMPT.into()],
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
                    return Ok(text.text);
                }
            }
        }
    }

    bail!("No text output returned from model")
}
