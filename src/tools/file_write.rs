use super::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;

pub struct FileWriteTool;

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &'static str { "file_write" }

    fn description(&self) -> &'static str {
        "Write content to a file. Creates the file and parent directories if they don't exist. \
         Overwrites the file if it already exists. Use file_read first to check existing content \
         when editing. For targeted edits, prefer file_edit instead."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": { "type": "string", "description": "Absolute or relative path to the file" },
                "content": { "type": "string", "description": "Content to write to the file" }
            },
            "required": ["file_path", "content"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let file_path = match input.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing 'file_path' parameter"),
        };
        let content = match input.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::err("Missing 'content' parameter"),
        };

        let resolved = resolve(file_path, &ctx.cwd);

        // Create parent directories
        if let Some(parent) = resolved.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return ToolResult::err(format!("Failed to create directories: {e}"));
            }
        }

        match tokio::fs::write(&resolved, content).await {
            Ok(()) => {
                let lines = content.lines().count();
                let bytes = content.len();
                ToolResult::ok(format!(
                    "Wrote {} lines ({bytes} bytes) to {}",
                    lines,
                    resolved.display()
                ))
            }
            Err(e) => ToolResult::err(format!("Failed to write {}: {e}", resolved.display())),
        }
    }
}

fn resolve(file_path: &str, cwd: &Path) -> std::path::PathBuf {
    let p = Path::new(file_path);
    if p.is_absolute() { p.to_path_buf() } else { cwd.join(p) }
}
