#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LlmProvider {
    #[default]
    OpenAI,
    Anthropic,
}

pub const LLM_PROVIDERS: [&str; 2] = [
    LlmProvider::OpenAI.as_str(),
    LlmProvider::Anthropic.as_str(),
];

impl LlmProvider {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "openai" => Some(Self::OpenAI),
            "anthropic" => Some(Self::Anthropic),
            _ => None,
        }
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAI => "openai",
            Self::Anthropic => "anthropic",
        }
    }

    pub const fn base_url(self) -> &'static str {
        match self {
            Self::OpenAI => "https://api.openai.com/v1/",
            Self::Anthropic => "https://api.anthropic.com/v1/",
        }
    }

    pub const fn default_model(self) -> &'static str {
        match self {
            Self::OpenAI => "gpt-5-nano",
            Self::Anthropic => "claude-3-sonnet",
        }
    }
}
