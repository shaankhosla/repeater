use anyhow::Result;
use async_openai::{Client, config::OpenAIConfig};

use super::response::request_single_text_response;

const REPHRASE_MODEL: &str = "gpt-5-nano";

const SYSTEM_PROMPT: &str = r#"
You rewrite flashcard questions to be clearer while keeping the same fact and difficulty.
Never reveal the answer inside the question and keep the tone neutral.
If there is no clear way to rewrite the question, return the original question verbatim.
"#;

pub async fn request_question_rephrase(
    client: &Client<OpenAIConfig>,
    question: &str,
    answer: &str,
) -> Result<String> {
    let user_prompt = format!(
        "Rewrite the question below so it is clearer, but keep the meaning the same.\n\
         Return only the rewritten question.\n\n\
         Question: {question}\n\
         Answer (for context; do not reveal): {answer}"
    );

    request_single_text_response(client, REPHRASE_MODEL, SYSTEM_PROMPT, &user_prompt).await
}
