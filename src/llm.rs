use std::io::{self, Write};

use anyhow::{Context, Result, anyhow};
use async_openai::types::{
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs,
};
use async_openai::{Client, config::OpenAIConfig};

const SERVICE: &str = "com.repeat.cli";
const USERNAME: &str = "openai";

use keyring::Entry;

const CLOZE_MODEL: &str = "gpt-4o-mini";

pub async fn ensure_client(user_prompt: &str) -> Result<Client<OpenAIConfig>> {
    let llm_key = load_api_key();
    let key = match llm_key {
        Ok(api_key) => api_key,
        Err(_) => {
            let api_key = prompt_user_for_key(user_prompt)?;
            if api_key.is_empty() {
                return Err(anyhow!(
                    "No OpenAI API key provided; cannot generate Cloze text automatically"
                ));
            }
            store_api_key(&api_key)?;
            api_key
        }
    };
    let client = initialize_client(&key).await?;
    healthcheck_client(&client).await?;
    Ok(client)
}

async fn initialize_client(api_key: &str) -> Result<Client<OpenAIConfig>> {
    let config = OpenAIConfig::new().with_api_key(api_key);

    let client = Client::with_config(config);
    Ok(client)
}

async fn healthcheck_client(client: &Client<OpenAIConfig>) -> Result<()> {
    client
        .models()
        .list()
        .await
        .context("Failed to list models")?;
    Ok(())
}

pub async fn request_cloze(client: &Client<OpenAIConfig>, text: &str) -> Result<String> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(CLOZE_MODEL)
        .max_tokens(200_u16)
        .temperature(0.2)
        .messages([
            ChatCompletionRequestSystemMessageArgs::default()
                .content("You convert flashcards into Cloze deletions. A Cloze deletion is denoted by square brackets: [hidden text]. Only add in one Cloze deletion. Your goal is to highlight the part of the flashcard you believe is most critical for a studying user to be able to recall. It can be a word or a small phrase.")
                .build()?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(format!(
                    "Turn the following text into a Cloze card by inserting [] around the hidden portion. Return the exact same text as below, but just with the addition of brackets around the Cloze deletion. For example, if you were shown the follwing text:\n\nC: Speech is produced in Broca's area.\n\nThis might be a good response to produce:\n\nC: Speech is produced in [Broca's] area.\n\nThis is the text you should generate the Cloze deletion for:
:\n{}",
                    text
                ))
                .build()?
                .into(),
        ])
        .build()?;

    let response = client
        .chat()
        .create(request)
        .await
        .context("LLM API failed to request Cloze generation")?;

    let output = response
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .ok_or_else(|| anyhow!("No content returned from model"))?;

    Ok(output)
}

fn prompt_user_for_key(prompt: &str) -> Result<String> {
    // let dim = "\x1b[2m";
    let reset = "\x1b[0m";
    // let cyan = "\x1b[36m";
    // let red = "\x1b[31m";
    let green = "\x1b[32m";
    // let blue = "\x1b[34m";

    println!("{}", prompt);
    println!(
        "{green}If you'd like to use an LLM to turn this into a Cloze, enter your OpenAI API{reset} key (https://platform.openai.com/account/api-keys) if you'd like to use this feature. It's stored locally for future use. Leave blank if not."
    );
    let _ = io::stdout().flush();

    let mut user_input = String::new();
    io::stdin().read_line(&mut user_input)?;
    let trimmed = user_input.trim();
    Ok(trimmed.to_string())
}

fn store_api_key(api_key: &str) -> Result<()> {
    let entry = Entry::new(SERVICE, USERNAME)?;
    entry.set_password(api_key)?;
    Ok(())
}

fn load_api_key() -> Result<String> {
    let entry = Entry::new(SERVICE, USERNAME)?;
    Ok(entry.get_password()?)
}
