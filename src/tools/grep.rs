use super::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::process::Stdio;
use tokio::process::Command;

pub struct GrepTool;

const MAX_RESULTS: usize = 50;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &'static str { "grep" }

    fn description(&self) -> &'static str {
        "Search for a pattern in files using ripgrep. Returns matching lines with file paths \
         and line numbers. Falls back to grep if ripgrep is not installed. Results are capped \
         at 50 matches."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Search pattern (regex supported)" },
                "path": { "type": "string", "description": "Directory or file to search (default: cwd)" },
                "include": { "type": "string", "description": "Glob pattern to filter files, e.g. '*.rs'" },
                "case_insensitive": { "type": "boolean", "description": "Case-insensitive search (default: false)" }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let pattern = match input.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing 'pattern'"),
        };

        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let include = input.get("include").and_then(|v| v.as_str());
        let case_insensitive = input
            .get("case_insensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Try ripgrep first, fall back to grep
        let (cmd, args) = build_command(pattern, path, include, case_insensitive);

        let result = Command::new(&cmd)
            .args(&args)
            .current_dir(&ctx.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<&str> = stdout.lines().take(MAX_RESULTS).collect();

                if lines.is_empty() {
                    return ToolResult::ok("No matches found.");
                }

                let total_matches = stdout.lines().count();
                let mut result = lines.join("\n");
                if total_matches > MAX_RESULTS {
                    result.push_str(&format!(
                        "\n... ({} more matches truncated)",
                        total_matches - MAX_RESULTS
                    ));
                }
                ToolResult::ok(result)
            }
            Err(_) => {
                // ripgrep not found, try grep
                let fallback = Command::new("grep")
                    .args(["-rn", pattern, path])
                    .current_dir(&ctx.cwd)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await;

                match fallback {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        if stdout.is_empty() {
                            ToolResult::ok("No matches found.")
                        } else {
                            let lines: Vec<&str> = stdout.lines().take(MAX_RESULTS).collect();
                            ToolResult::ok(lines.join("\n"))
                        }
                    }
                    Err(e) => ToolResult::err(format!("Search failed: {e}")),
                }
            }
        }
    }
}

fn build_command(
    pattern: &str,
    path: &str,
    include: Option<&str>,
    case_insensitive: bool,
) -> (String, Vec<String>) {
    let mut args = vec!["-n".to_string(), "--no-heading".to_string()];
    if case_insensitive {
        args.push("-i".to_string());
    }
    if let Some(glob) = include {
        args.push("-g".to_string());
        args.push(glob.to_string());
    }
    args.push(pattern.to_string());
    args.push(path.to_string());
    ("rg".to_string(), args)
}
