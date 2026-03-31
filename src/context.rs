use std::path::{Path, PathBuf};

/// Load system prompt: base instructions + optional CLAUDE.md / memory files + git status
pub async fn build_system_prompt(
    cwd: &Path,
    custom_prompt: Option<&str>,
    load_memory: bool,
) -> String {
    let mut parts: Vec<String> = vec![base_prompt().to_string()];

    if let Some(custom) = custom_prompt {
        parts.push(format!("# User Instructions\n\n{custom}"));
    }

    if load_memory {
        if let Some(memory) = load_memory_files(cwd).await {
            parts.push(format!("# Project Context\n\n{memory}"));
        }
    }

    if let Some(git) = git_status(cwd).await {
        parts.push(format!("# Git Status\n\n{git}"));
    }

    // Current date
    parts.push(format!(
        "Today's date is {}.",
        chrono::Local::now().format("%Y-%m-%d")
    ));

    parts.join("\n\n")
}

fn base_prompt() -> &'static str {
    r#"You are an expert software engineer assistant. You help users with programming tasks by reading, writing, and editing files, running shell commands, and searching codebases.

## Guidelines
- Be concise and direct.
- When editing files, use file_edit for targeted changes and file_write only for new files or full rewrites.
- Always read a file before editing it.
- Run tests after making changes when a test suite exists.
- Prefer single-purpose shell commands; chain with && for sequential operations.
- When searching, use grep for content and glob for file discovery.

## Tool Use
- Use tools proactively to gather information and make changes.
- Don't ask the user to do things you can do with tools.
- After making code changes, verify them by running appropriate commands."#
}

/// Walk up from cwd looking for CLAUDE.md, .claude/instructions.md, or similar memory files
async fn load_memory_files(cwd: &Path) -> Option<String> {
    let candidates = [
        "CLAUDE.md",
        ".claude/instructions.md",
        "AGENTS.md",
        ".cursorrules",
        ".github/copilot-instructions.md",
    ];

    let mut found = Vec::new();
    let mut dir = cwd.to_path_buf();

    // Walk up at most 5 levels
    for _ in 0..5 {
        for name in &candidates {
            let path = dir.join(name);
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                if !content.trim().is_empty() {
                    found.push(format!(
                        "<!-- {} -->\n{}",
                        path.strip_prefix(cwd).unwrap_or(&path).display(),
                        content.trim()
                    ));
                }
            }
        }
        if !dir.pop() {
            break;
        }
    }

    if found.is_empty() {
        None
    } else {
        Some(found.join("\n\n---\n\n"))
    }
}

/// Get a snapshot of git status for context
async fn git_status(cwd: &Path) -> Option<String> {
    let is_git = is_git_repo(cwd).await;
    if !is_git {
        return None;
    }

    let branch = run_git(cwd, &["branch", "--show-current"]).await;
    let status = run_git(cwd, &["status", "--short"]).await;
    let log = run_git(cwd, &["log", "--oneline", "-n", "5"]).await;

    let mut parts = vec!["Git status snapshot (may be stale):".to_string()];

    if let Some(b) = branch {
        parts.push(format!("Branch: {b}"));
    }
    if let Some(s) = &status {
        let display = if s.len() > 2000 {
            format!("{}... (truncated)", &s[..2000])
        } else {
            s.clone()
        };
        parts.push(format!("Status:\n{display}"));
    }
    if let Some(l) = log {
        parts.push(format!("Recent commits:\n{l}"));
    }

    Some(parts.join("\n\n"))
}

async fn is_git_repo(cwd: &Path) -> bool {
    run_git(cwd, &["rev-parse", "--is-inside-work-tree"])
        .await
        .is_some()
}

async fn run_git(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .ok()?;

    if output.status.success() {
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    } else {
        None
    }
}

/// Resolve the session storage directory
pub fn sessions_dir() -> PathBuf {
    let base = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("vertebric")
        .join("sessions");
    let _ = std::fs::create_dir_all(&base);
    base
}
