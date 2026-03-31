use dialoguer::{theme::ColorfulTheme, Select};
use std::path::PathBuf;

/// Supported LLM providers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    Claude,
    OpenAI,
    Gemini,
    Custom,
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

    pub fn default_base_url(&self) -> &'static str {
        match self {
            Self::Claude => "https://api.anthropic.com",
            Self::OpenAI => "https://api.openai.com",
            Self::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai",
            Self::Custom => "http://localhost:11434",
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

    fn base_url_env(&self) -> &'static str {
        match self {
            Self::Claude => "ANTHROPIC_BASE_URL",
            Self::OpenAI => "OPENAI_BASE_URL",
            Self::Gemini => "GEMINI_BASE_URL",
            Self::Custom => "CUSTOM_API_BASE",
        }
    }

    pub fn available_models(&self) -> Vec<(&'static str, &'static str)> {
        match self {
            Self::Claude => vec![
                ("claude-sonnet-4-20250514", "Claude Sonnet 4"),
                ("claude-opus-4-20250514", "Claude Opus 4"),
                ("claude-haiku-4-20250514", "Claude Haiku 4"),
            ],
            Self::OpenAI => vec![
                ("gpt-4o", "GPT-4o"),
                ("gpt-4o-mini", "GPT-4o Mini"),
                ("o3", "o3"),
                ("o4-mini", "o4-mini"),
            ],
            Self::Gemini => vec![
                ("gemini-2.5-flash", "Gemini 2.5 Flash"),
                ("gemini-2.5-pro", "Gemini 2.5 Pro"),
                ("gemini-2.0-flash", "Gemini 2.0 Flash"),
            ],
            Self::Custom => vec![
                ("default", "Default"),
            ],
        }
    }
}

const PROVIDER_OPTIONS: &[(&str, fn() -> Provider)] = &[
    ("Claude (Anthropic)", || Provider::Claude),
    ("OpenAI", || Provider::OpenAI),
    ("Gemini (Google)", || Provider::Gemini),
    ("Custom (OpenAI-compatible)", || Provider::Custom),
];

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
    /// CLI-driven path: provider and model explicitly set via flags.
    pub fn from_args(args: &crate::cli::Args) -> anyhow::Result<Self> {
        let provider = Provider::from_str(&args.provider);
        let model = args
            .model
            .clone()
            .unwrap_or_else(|| provider.available_models()[0].0.to_string());

        let base_url = std::env::var(provider.base_url_env())
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

    /// Interactive path: select provider → enter key → select model.
    pub fn from_interactive(args: &crate::cli::Args) -> anyhow::Result<Self> {
        let theme = ColorfulTheme::default();

        // 1. Select provider
        let labels: Vec<&str> = PROVIDER_OPTIONS.iter().map(|(l, _)| *l).collect();
        let idx = Select::with_theme(&theme)
            .with_prompt("Select provider")
            .items(&labels)
            .default(0)
            .interact()?;
        let provider = PROVIDER_OPTIONS[idx].1();

        // 2. Resolve API key
        let api_key = if provider == Provider::Custom {
            std::env::var(provider.api_key_env()).unwrap_or_default()
        } else {
            crate::auth::get_or_prompt_api_key(&provider)?
        };

        // 3. Select model
        let models = provider.available_models();
        let model_labels: Vec<&str> = models.iter().map(|(_, d)| *d).collect();
        let model_idx = Select::with_theme(&theme)
            .with_prompt("Select model")
            .items(&model_labels)
            .default(0)
            .interact()?;
        let model = models[model_idx].0.to_string();

        let base_url = std::env::var(provider.base_url_env())
            .unwrap_or_else(|_| provider.default_base_url().to_string());
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
