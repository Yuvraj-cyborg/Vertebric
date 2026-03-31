use super::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;

pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &'static str { "file_read" }

    fn description(&self) -> &'static str {
        "Read the contents of a file. Returns file content with line numbers. \
         Supports offset and limit to read specific ranges. Binary files are detected \
         and rejected. Use this before editing to understand existing code."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": { "type": "string", "description": "Absolute or relative path to the file" },
                "offset": { "type": "integer", "description": "Line number to start reading from (1-indexed, default: 1)" },
                "limit": { "type": "integer", "description": "Max number of lines to read (default: all)" }
            },
            "required": ["file_path"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let file_path = match input.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing 'file_path' parameter"),
        };

        let resolved = resolve_path(file_path, &ctx.cwd);
        let offset = input.get("offset").and_then(|v| v.as_u64()).unwrap_or(1).max(1) as usize;
        let limit = input.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);

        match tokio::fs::read_to_string(&resolved).await {
            Ok(content) => {
                // Check for binary
                if content.chars().take(512).any(|c| c == '\0') {
                    return ToolResult::err("Binary file detected — cannot read");
                }

                let lines: Vec<&str> = content.lines().collect();
                let total = lines.len();
                let start = (offset - 1).min(total);
                let end = match limit {
                    Some(l) => (start + l).min(total),
                    None => total,
                };

                let mut output = String::new();
                for (i, line) in lines[start..end].iter().enumerate() {
                    let line_num = start + i + 1;
                    output.push_str(&format!("{line_num:>5}: {line}\n"));
                }

                if output.is_empty() {
                    output = "(empty file)".to_string();
                }

                let header = format!("File: {} ({total} lines)\n", resolved.display());
                ToolResult::ok(format!("{header}{output}"))
            }
            Err(e) => ToolResult::err(format!("Failed to read {}: {e}", resolved.display())),
        }
    }
}

fn resolve_path(file_path: &str, cwd: &Path) -> std::path::PathBuf {
    let p = Path::new(file_path);
    if p.is_absolute() { p.to_path_buf() } else { cwd.join(p) }
}
