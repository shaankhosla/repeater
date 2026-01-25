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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_providers() {
        assert_eq!(LlmProvider::parse("openai"), Some(LlmProvider::OpenAI));
        assert_eq!(LlmProvider::parse("OpenAI"), Some(LlmProvider::OpenAI));
        assert_eq!(
            LlmProvider::parse(" anthropic "),
            Some(LlmProvider::Anthropic)
        );
    }

    #[test]
    fn parse_invalid_provider() {
        assert_eq!(LlmProvider::parse("unknown"), None);
        assert_eq!(LlmProvider::parse(""), None);
    }

    #[test]
    fn as_str_matches_expected() {
        assert_eq!(LlmProvider::OpenAI.as_str(), "openai");
        assert_eq!(LlmProvider::Anthropic.as_str(), "anthropic");
    }

    #[test]
    fn base_url_matches_expected() {
        assert_eq!(LlmProvider::OpenAI.base_url(), "https://api.openai.com/v1/");
        assert_eq!(
            LlmProvider::Anthropic.base_url(),
            "https://api.anthropic.com/v1/"
        );
    }

    #[test]
    fn default_model_matches_expected() {
        assert_eq!(LlmProvider::OpenAI.default_model(), "gpt-5-nano");
        assert_eq!(LlmProvider::Anthropic.default_model(), "claude-3-sonnet");
    }

    #[test]
    fn llm_providers_constant_is_consistent() {
        assert_eq!(LLM_PROVIDERS, ["openai", "anthropic"]);
    }
}
