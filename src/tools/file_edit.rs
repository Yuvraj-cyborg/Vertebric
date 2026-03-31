use super::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;

pub struct FileEditTool;

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &'static str { "file_edit" }

    fn description(&self) -> &'static str {
        "Make targeted edits to an existing file by specifying exact text to find and replace. \
         Only performs the replacement if the target text is found exactly once in the file \
         (unless allow_multiple is true). Use file_read first to see the current content."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": { "type": "string", "description": "Path to the file to edit" },
                "target": { "type": "string", "description": "Exact text to find in the file" },
                "replacement": { "type": "string", "description": "Text to replace the target with" },
                "allow_multiple": { "type": "boolean", "description": "Allow replacing multiple occurrences (default: false)" }
            },
            "required": ["file_path", "target", "replacement"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let file_path = match input.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing 'file_path'"),
        };
        let target = match input.get("target").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return ToolResult::err("Missing 'target'"),
        };
        let replacement = match input.get("replacement").and_then(|v| v.as_str()) {
            Some(r) => r,
            None => return ToolResult::err("Missing 'replacement'"),
        };
        let allow_multiple = input
            .get("allow_multiple")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let resolved = resolve(file_path, &ctx.cwd);

        let content = match tokio::fs::read_to_string(&resolved).await {
            Ok(c) => c,
            Err(e) => return ToolResult::err(format!("Cannot read {}: {e}", resolved.display())),
        };

        let count = content.matches(target).count();

        if count == 0 {
            return ToolResult::err(format!(
                "Target text not found in {}. Use file_read to check the current content.",
                resolved.display()
            ));
        }
        if count > 1 && !allow_multiple {
            return ToolResult::err(format!(
                "Found {count} occurrences of target text. Set allow_multiple=true or make the target more specific."
            ));
        }

        let new_content = content.replace(target, replacement);

        match tokio::fs::write(&resolved, &new_content).await {
            Ok(()) => ToolResult::ok(format!(
                "Replaced {count} occurrence(s) in {}",
                resolved.display()
            )),
            Err(e) => ToolResult::err(format!("Failed to write {}: {e}", resolved.display())),
        }
    }
}

fn resolve(file_path: &str, cwd: &Path) -> std::path::PathBuf {
    let p = Path::new(file_path);
    if p.is_absolute() { p.to_path_buf() } else { cwd.join(p) }
}
