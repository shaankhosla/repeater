#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LlmProvider {
    #[default]
    OpenAI,
    Anthropic,
    Groq,
    OpenRouter,
    Google,
}

pub const LLM_PROVIDERS: [&str; 5] = [
    LlmProvider::OpenAI.as_str(),
    LlmProvider::Anthropic.as_str(),
    LlmProvider::Groq.as_str(),
    LlmProvider::OpenRouter.as_str(),
    LlmProvider::Google.as_str(),
];

impl LlmProvider {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "openai" => Some(Self::OpenAI),
            "anthropic" => Some(Self::Anthropic),
            "groq" => Some(Self::Groq),
            "openrouter" => Some(Self::OpenRouter),
            "google" => Some(Self::Google),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAI => "openai",
            Self::Anthropic => "anthropic",
            Self::Groq => "groq",
            Self::OpenRouter => "openrouter",
            Self::Google => "google",
        }
    }

    pub const fn base_url(self) -> &'static str {
        match self {
            Self::OpenAI => "https://api.openai.com/v1/",
            Self::Anthropic => "https://api.anthropic.com/v1/",
            Self::Groq => "https://api.groq.com/openai/v1/",
            Self::OpenRouter => "https://openrouter.ai/api/v1/",
            Self::Google => "https://generativelanguage.googleapis.com/v1beta/openai/",
        }
    }

    pub const fn default_model(self) -> &'static str {
        match self {
            Self::OpenAI => "gpt-5-nano",
            Self::Anthropic => "claude-sonnet-4-6",
            Self::Groq => "llama-3.3-70b-versatile",
            Self::OpenRouter => "anthropic/claude-sonnet-4-6",
            Self::Google => "gemini-2.5-flash",
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
        assert_eq!(LlmProvider::parse("groq"), Some(LlmProvider::Groq));
        assert_eq!(
            LlmProvider::parse("OpenRouter"),
            Some(LlmProvider::OpenRouter)
        );
        assert_eq!(LlmProvider::parse("google"), Some(LlmProvider::Google));
    }

    #[test]
    fn parse_invalid_provider() {
        assert_eq!(LlmProvider::parse("unknown"), None);
        assert_eq!(LlmProvider::parse(""), None);
    }

    #[test]
    fn roundtrip_as_str_parse() {
        for &name in &LLM_PROVIDERS {
            assert!(
                LlmProvider::parse(name).is_some(),
                "{name} should roundtrip through parse"
            );
        }
    }

    #[test]
    fn all_providers_have_base_url() {
        for &name in &LLM_PROVIDERS {
            let provider = LlmProvider::parse(name).unwrap();
            assert!(
                provider.base_url().starts_with("https://"),
                "{name} base_url should be https"
            );
        }
    }

    #[test]
    fn llm_providers_constant_is_consistent() {
        assert_eq!(LLM_PROVIDERS.len(), 5);
        assert_eq!(LLM_PROVIDERS[0], "openai");
    }
}
