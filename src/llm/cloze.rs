use anyhow::Result;
use async_openai::{Client, config::OpenAIConfig};

use super::response::request_single_text_response;

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
It can be a word or a small phrase. For example, if you were shown the following text:

C: Speech is produced in Broca's area.

This might be a good response to produce:

C: Speech is produced in [Broca's] area.

This is the text you should generate the Cloze deletion for:

"#;

pub async fn request_cloze(client: &Client<OpenAIConfig>, text: &str) -> Result<String> {
    let user_prompt = format!("{USER_PROMPT_HEADER}{text}");

    request_single_text_response(client, CLOZE_MODEL, SYSTEM_PROMPT, &user_prompt).await
}
