use super::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::process::Stdio;
use tokio::process::Command;

const MAX_OUTPUT: usize = 30_000;
const DEFAULT_TIMEOUT_MS: u64 = 120_000;

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &'static str {
        "bash"
    }

    fn description(&self) -> &'static str {
        "Execute a bash command in the shell. Use for running scripts, installing packages, \
         compiling code, managing files/git, and any system operations. Commands run in the \
         user's working directory. Long-running commands will be killed after the timeout. \
         Prefer single concise commands; chain with && if needed."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in milliseconds (default 120000)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let command = match input.get("command").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::err("Missing 'command' parameter"),
        };

        let timeout_ms = input
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_TIMEOUT_MS);

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            run_command(command, &ctx.cwd),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                let mut combined = String::new();
                if !output.stdout.is_empty() {
                    combined.push_str(&output.stdout);
                }
                if !output.stderr.is_empty() {
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str("STDERR:\n");
                    combined.push_str(&output.stderr);
                }
                if combined.is_empty() {
                    combined = "(no output)".to_string();
                }
                // Truncate massive outputs
                if combined.len() > MAX_OUTPUT {
                    combined.truncate(MAX_OUTPUT);
                    combined.push_str("\n... (output truncated)");
                }
                let is_error = output.exit_code != 0;
                if is_error {
                    combined = format!("Exit code: {}\n{combined}", output.exit_code);
                }
                ToolResult { content: combined, is_error }
            }
            Ok(Err(e)) => ToolResult::err(format!("Command failed: {e}")),
            Err(_) => ToolResult::err(format!(
                "Command timed out after {}ms",
                timeout_ms
            )),
        }
    }
}

struct CommandOutput {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

async fn run_command(
    command: &str,
    cwd: &std::path::Path,
) -> anyhow::Result<CommandOutput> {
    let output = Command::new("bash")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    Ok(CommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}
