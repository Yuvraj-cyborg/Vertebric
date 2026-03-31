use clap::Parser;
use colored::Colorize;
use std::io::{self, BufRead, Write};

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

fn read_prompt() -> Option<String> {
    eprint!("{}", "> ".bold().cyan());
    io::stderr().flush().ok();
    let mut line = String::new();
    match io::stdin().lock().read_line(&mut line) {
        Ok(0) => None,
        Ok(_) => {
            let trimmed = line.trim().to_string();
            if trimmed.is_empty() { None } else { Some(trimmed) }
        }
        Err(_) => None,
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let config = if args.model.is_some() {
        Config::from_args(&args)
    } else {
        Config::from_interactive(&args)
    };

    let config = match config {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} {e}", "Error:".red().bold());
            std::process::exit(1);
        }
    };

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

    let system = context::build_system_prompt(
        &config.cwd,
        config.system_prompt.as_deref(),
        !config.disable_memory_files,
    )
    .await;

    let mut engine = match Engine::new(config, system) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{} Failed to initialize: {e}", "Error:".red().bold());
            std::process::exit(1);
        }
    };

    // If -p was given, run that single prompt then enter the loop
    if let Some(p) = &args.prompt {
        match run_prompt(&mut engine, p).await {
            LoopAction::Continue => {}
            LoopAction::Exit(code) => std::process::exit(code),
        }
    }

    // Interactive REPL
    eprintln!("{}", "Type a prompt, or Ctrl-C / Ctrl-D to exit.".dimmed());
    loop {
        match read_prompt() {
            Some(prompt) => match run_prompt(&mut engine, &prompt).await {
                LoopAction::Continue => {}
                LoopAction::Exit(code) => {
                    eprintln!("{}", engine.cost_summary().dimmed());
                    std::process::exit(code);
                }
            },
            None => {
                eprintln!("\n{}", engine.cost_summary().dimmed());
                break;
            }
        }
    }
}

enum LoopAction {
    Continue,
    Exit(i32),
}

async fn run_prompt(engine: &mut Engine, prompt: &str) -> LoopAction {
    let result = engine.run(prompt).await;
    eprintln!("{}", engine.cost_summary().dimmed());

    match result {
        EngineResult::Done(_) => LoopAction::Continue,
        EngineResult::MaxTurns => LoopAction::Continue,
        EngineResult::MaxBudget => {
            eprintln!("{}", "Budget exhausted.".yellow());
            LoopAction::Exit(0)
        }
        EngineResult::Error(e) => {
            eprintln!("{} {e}", "Error:".red().bold());
            LoopAction::Continue
        }
    }
}
