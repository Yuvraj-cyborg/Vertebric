use std::path::PathBuf;

/// Supported LLM providers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    Claude,
    OpenAI,
    Gemini,
    Custom, // any OpenAI-compatible endpoint
}

impl Provider {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "claude" | "anthropic" => Self::Claude,
            "openai" | "gpt" => Self::OpenAI,
            "gemini" | "google" => Self::Gemini,
            _ => Self::Custom,
        }
    }

    pub fn default_model(&self) -> &'static str {
        match self {
            Self::Claude => "claude-sonnet-4-20250514",
            Self::OpenAI => "gpt-4o",
            Self::Gemini => "gemini-2.5-flash",
            Self::Custom => "default",
        }
    }

    pub fn default_base_url(&self) -> &'static str {
        match self {
            Self::Claude => "https://api.anthropic.com",
            Self::OpenAI => "https://api.openai.com",
            Self::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai",
            Self::Custom => "http://localhost:11434", // ollama default
        }
    }

    pub fn api_key_env(&self) -> &'static str {
        match self {
            Self::Claude => "ANTHROPIC_API_KEY",
            Self::OpenAI => "OPENAI_API_KEY",
            Self::Gemini => "GEMINI_API_KEY",
            Self::Custom => "CUSTOM_API_KEY",
        }
    }

    pub fn provider_display(&self) -> &'static str {
        match self {
            Self::Claude => "Claude (Anthropic)",
            Self::OpenAI => "OpenAI",
            Self::Gemini => "Gemini (Google)",
            Self::Custom => "Custom API",
        }
    }
}

/// Full runtime configuration built from CLI args + env vars
#[derive(Debug, Clone)]
pub struct Config {
    pub provider: Provider,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub max_tokens: u32,
    pub max_turns: Option<u32>,
    pub max_budget_usd: Option<f64>,
    pub cwd: PathBuf,
    pub system_prompt: Option<String>,
    pub verbose: bool,
    pub disable_memory_files: bool,
}

impl Config {
    pub fn from_args(args: &crate::cli::Args) -> anyhow::Result<Self> {
        let provider = Provider::from_str(&args.provider);

        let model = args
            .model
            .clone()
            .unwrap_or_else(|| provider.default_model().to_string());

        let base_url = std::env::var(match &provider {
            Provider::Claude => "ANTHROPIC_BASE_URL",
            Provider::OpenAI => "OPENAI_BASE_URL",
            Provider::Gemini => "GEMINI_BASE_URL",
            Provider::Custom => "CUSTOM_API_BASE",
        })
        .unwrap_or_else(|_| provider.default_base_url().to_string());

        let api_key = if provider == Provider::Custom {
            std::env::var(provider.api_key_env()).unwrap_or_default()
        } else {
            crate::auth::get_or_prompt_api_key(&provider)?
        };

        let cwd = std::env::current_dir()?;

        Ok(Self {
            provider,
            model,
            base_url,
            api_key,
            max_tokens: args.max_tokens.unwrap_or(16384),
            max_turns: args.max_turns,
            max_budget_usd: args.max_budget,
            cwd,
            system_prompt: args.system_prompt.clone(),
            verbose: args.verbose,
            disable_memory_files: args.no_memory,
        })
    }

}
