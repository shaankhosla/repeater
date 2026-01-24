use crate::utils::trim_line;

#[derive(Debug, Clone)]
pub struct LlmProvider {
    pub name: &'static str,
    pub base_url: &'static str,
}

const DEFAULT_PROVIDER: LlmProvider = LlmProvider {
    name: "openai",
    base_url: "https://api.openai.com/v1/",
};

pub const LLM_PROVIDERS: &[LlmProvider] = &[
    LlmProvider {
        name: "openai",
        base_url: "https://api.openai.com/v1/",
    },
    LlmProvider {
        name: "anthropic",
        base_url: "https://api.anthropic.com/v1/",
    },
];

pub fn get_llm_base_url(provider_name: &str) -> Option<String> {
    let provider_name = trim_line(provider_name)?;

    LLM_PROVIDERS
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(provider_name))
        .map(|p| p.base_url.to_string())
}
