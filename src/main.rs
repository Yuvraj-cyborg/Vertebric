use clap::Parser;
use colored::Colorize;

mod api;
mod auth;
mod cli;
mod config;
mod context;
mod cost;
mod engine;
mod session;
mod tools;
mod tui;
mod types;

use cli::Args;
use config::Config;
use engine::{Engine, EngineResult};

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let config = match Config::from_args(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} {e}", "Error:".red().bold());
            std::process::exit(1);
        }
    };

    let prompt = match &args.prompt {
        Some(p) => p.clone(),
        None => {
            eprintln!("{} Provide a prompt with -p/--prompt", "Error:".red().bold());
            eprintln!("  Example: vertebric -p \"fix the bug in main.rs\"");
            std::process::exit(1);
        }
    };

    // Print header
    eprintln!(
        "{}",
        format!(
            "vertebric v{} | {} | {}",
            env!("CARGO_PKG_VERSION"),
            config.provider.provider_display(),
            config.model,
        )
        .dimmed()
    );

    // Build system prompt
    let system = context::build_system_prompt(
        &config.cwd,
        config.system_prompt.as_deref(),
        !config.disable_memory_files,
    )
    .await;

    // Create and run engine
    let mut engine = match Engine::new(config, system) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{} Failed to initialize: {e}", "Error:".red().bold());
            std::process::exit(1);
        }
    };

    let result = engine.run(&prompt).await;

    // Print summary
    eprintln!("{}", engine.cost_summary().dimmed());

    match result {
        EngineResult::Done(_) => {}
        EngineResult::MaxTurns => std::process::exit(0),
        EngineResult::MaxBudget => std::process::exit(0),
        EngineResult::Error(e) => {
            eprintln!("{} {e}", "Error:".red().bold());
            std::process::exit(1);
        }
    }
}
