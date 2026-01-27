use std::env;

use dialoguer::FuzzySelect;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use dialoguer::Input;
use dialoguer::Select;
use dialoguer::{Password, theme::ColorfulTheme};
use serde::{Deserialize, Serialize};

use crate::llm::client::get_models;
use crate::llm::client::initialize_client;
use crate::llm::provider::LLM_PROVIDERS;
use crate::palette::Palette;
use crate::utils::get_data_dir;
use crate::utils::strip_controls_and_escapes;
use crate::utils::trim_line;

use super::provider::LlmProvider;

pub const API_KEY_ENV: &str = "OPENAI_API_KEY";

const AUTH_FILE_NAME: &str = "auth.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeySource {
    Environment,
    AuthFile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAuth {
    pub name: String,
    pub key: Option<String>,
    pub base_url: String,
    pub model: String,
}

impl ApiKeySource {
    pub fn description(&self) -> &'static str {
        match self {
            ApiKeySource::Environment => "environment variable",
            ApiKeySource::AuthFile => "local auth file",
        }
    }
}

#[cfg(test)]
const TEST_AUTH_PATH_ENV: &str = "REPEATER_TEST_AUTH_PATH";

pub fn clear_api_key() -> Result<bool> {
    let auth_path = auth_file_path()?;

    match fs::remove_file(&auth_path) {
        Ok(()) => Ok(true), // existed and was cleared
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(false) // did not exist
        }
        Err(e) => {
            Err(e).with_context(|| format!("Failed to remove auth file at {}", auth_path.display()))
        }
    }
}

pub async fn prompt_for_llm_details(prompt: &str) -> Result<ProviderAuth> {
    let mut provider_names: Vec<String> = LLM_PROVIDERS
        .iter()
        .map(|p| {
            let mut chars = p.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect();

    let other_label = "Other (must be OpenAI API compatible)";
    provider_names.push(other_label.to_string());

    println!("\n{}", Palette::paint(Palette::ACCENT, prompt));
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select the LLM provider:")
        .default(0)
        .items(&provider_names)
        .interact()
        .unwrap();
    let chosen_provider = &provider_names[selection];

    let base_url = if chosen_provider == other_label {
        let raw_base_url: String = Input::new()
            .with_prompt("Enter base URL (e.g. https://api.openai.com/v1/)")
            .interact_text()
            .unwrap();

        let cleaned_base_url = strip_controls_and_escapes(&raw_base_url);
        let no_slashes = cleaned_base_url.trim_end_matches("/");

        trim_line(&no_slashes)
            .with_context(|| "Enter a valid URL, e.g. https://api.openai.com/v1/")?
            .to_string()
    } else {
        let llm_provider = LlmProvider::parse(chosen_provider).unwrap();
        llm_provider.base_url().to_string()
    };

    let raw_password = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("Enter your API Key (leave blank to skip)")
        .allow_empty_password(true)
        .interact()
        .unwrap();

    let password = strip_controls_and_escapes(&raw_password).trim().to_string();
    let mut auth = ProviderAuth {
        name: chosen_provider.to_string(),
        key: Some(password),
        base_url,
        model: "".to_string(),
    };
    let client = initialize_client(&auth)?;
    let models = get_models(&client)
        .await
        .with_context(|| "Failed to connect to provider")?;

    let model_selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select model to use (type to search):")
        .default(0)
        .items(&models)
        .interact()
        .unwrap();
    let selected_model = &models[model_selection];
    auth.model = selected_model.to_string();
    Ok(auth)
}

#[derive(Debug)]
pub struct ApiKeyLookup {
    pub llm_auth: Option<ProviderAuth>,
    pub source: Option<ApiKeySource>,
}

pub fn store_llm_details(llm_auth: &ProviderAuth) -> Result<()> {
    let auth_path = auth_file_path()?;
    write_auth_file(&auth_path, llm_auth)
}

pub fn get_api_key_from_sources() -> Result<ApiKeyLookup> {
    // 1. Environment variable
    if let Ok(value) = env::var(API_KEY_ENV)
        && !value.trim().is_empty()
    {
        let default_model = LlmProvider::default();
        return Ok(ApiKeyLookup {
            llm_auth: Some(ProviderAuth {
                key: Some(value.trim().to_string()),
                name: default_model.as_str().to_string(),
                base_url: default_model.base_url().to_string(),
                model: default_model.default_model().to_string(),
            }),
            source: Some(ApiKeySource::Environment),
        });
    }

    // 2. Auth file
    let auth_path = auth_file_path()?;
    Ok(
        match read_auth_file(&auth_path)
            .with_context(|| "To reset your LLM config, run `repeater llm key --set`.")?
        {
            Some(llm_auth) => ApiKeyLookup {
                llm_auth: Some(llm_auth),
                source: Some(ApiKeySource::AuthFile),
            },
            None => ApiKeyLookup {
                llm_auth: None,
                source: None,
            },
        },
    )
}

fn auth_file_path() -> Result<PathBuf> {
    #[cfg(test)]
    {
        if let Ok(path) = env::var(TEST_AUTH_PATH_ENV)
            && !path.trim().is_empty()
        {
            return Ok(PathBuf::from(path));
        }
    }

    let data_dir = get_data_dir()?;
    Ok(data_dir.join(AUTH_FILE_NAME))
}

fn read_auth_file(path: &Path) -> Result<Option<ProviderAuth>> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            if contents.trim().is_empty() {
                return Ok(None);
            }

            let parsed: ProviderAuth = serde_json::from_str(&contents)
                .with_context(|| format!("Failed to parse auth file at {}", path.display()))?;

            Ok(Some(parsed))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => {
            Err(err).with_context(|| format!("Failed to read auth file at {}", path.display()))
        }
    }
}

fn write_auth_file(path: &Path, value: &ProviderAuth) -> Result<()> {
    let contents = serialize_auth(value)?;
    fs::write(path, contents)
        .with_context(|| format!("Failed to write auth file at {}", path.display()))?;
    Ok(())
}

fn serialize_auth(value: &ProviderAuth) -> Result<String> {
    let contents = serde_json::to_string_pretty(value)?;
    Ok(format!("{}\n", contents))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parse_auth_contents_handles_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("auth.json");
        fs::write(&path, "\n\n").unwrap();

        let parsed = read_auth_file(&path);
        assert!(parsed.is_ok());
        assert!(parsed.unwrap().is_none());
    }

    #[test]
    fn file_doesnt_exist() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let unexisting_file_auth = read_auth_file(&path).unwrap();
        assert!(unexisting_file_auth.is_none());
    }

    #[test]
    fn overwrite() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("auth.json");

        unsafe {
            env::set_var(TEST_AUTH_PATH_ENV, &path);
        }
        let mut llm_auth = ProviderAuth {
            key: Some("fake_key".to_string()),
            name: "name".to_string(),
            base_url: "base_url".to_string(),
            model: "model".to_string(),
        };
        store_llm_details(&llm_auth).unwrap();

        llm_auth.key = Some("real_key".to_string());
        store_llm_details(&llm_auth).unwrap();

        let api_key = get_api_key_from_sources().unwrap();
        assert_eq!(api_key.llm_auth.unwrap().key.unwrap(), "real_key");

        clear_api_key().unwrap();

        let api_key = get_api_key_from_sources().unwrap();
        assert!(api_key.llm_auth.is_none());
    }

    #[test]
    fn load_key_without_store() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("auth.json");

        unsafe {
            env::set_var(TEST_AUTH_PATH_ENV, &path);
        }

        let api_key = get_api_key_from_sources().unwrap();
        assert!(api_key.llm_auth.is_none());

        let clear = clear_api_key().unwrap();
        assert!(!clear);
    }
}
