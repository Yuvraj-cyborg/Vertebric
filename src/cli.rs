use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "vertebric", about = "Multi-provider agentic coding CLI")]
pub struct Args {
    /// Prompt to send (print mode — non-interactive)
    #[arg(short, long)]
    pub prompt: Option<String>,

    /// LLM provider: claude, openai, or custom
    #[arg(long, default_value = "claude")]
    pub provider: String,

    /// Model name (provider-specific)
    #[arg(short, long)]
    pub model: Option<String>,

    /// Max output tokens per API call
    #[arg(long)]
    pub max_tokens: Option<u32>,

    /// Max agentic turns before stopping
    #[arg(long)]
    pub max_turns: Option<u32>,

    /// Max budget in USD before stopping
    #[arg(long)]
    pub max_budget: Option<f64>,

    /// Custom system prompt (overrides default)
    #[arg(long)]
    pub system_prompt: Option<String>,

    /// Don't load CLAUDE.md / memory files
    #[arg(long)]
    pub no_memory: bool,

    /// Verbose output (show per-turn cost, tool results)
    #[arg(short, long)]
    pub verbose: bool,
}
