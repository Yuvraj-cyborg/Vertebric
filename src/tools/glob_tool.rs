use super::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::Value;

pub struct GlobTool;

const MAX_RESULTS: usize = 200;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &'static str { "glob" }

    fn description(&self) -> &'static str {
        "Find files matching a glob pattern. Returns matching file paths relative to the \
         working directory. Use for discovering files before reading or editing them."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Glob pattern, e.g. 'src/**/*.rs' or '*.json'" },
                "path": { "type": "string", "description": "Base directory to search from (default: cwd)" }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let pattern = match input.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing 'pattern'"),
        };

        let base = input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| ctx.cwd.join(p))
            .unwrap_or_else(|| ctx.cwd.clone());

        let full_pattern = base.join(pattern).to_string_lossy().to_string();

        match glob::glob(&full_pattern) {
            Ok(entries) => {
                let mut paths: Vec<String> = Vec::new();
                let mut total = 0usize;

                for entry in entries.flatten() {
                    total += 1;
                    if paths.len() < MAX_RESULTS {
                        let display = entry
                            .strip_prefix(&ctx.cwd)
                            .unwrap_or(&entry)
                            .to_string_lossy()
                            .to_string();
                        paths.push(display);
                    }
                }

                if paths.is_empty() {
                    return ToolResult::ok("No files matched.");
                }

                let mut output = paths.join("\n");
                if total > MAX_RESULTS {
                    output.push_str(&format!("\n... ({} more files)", total - MAX_RESULTS));
                }
                ToolResult::ok(output)
            }
            Err(e) => ToolResult::err(format!("Invalid glob pattern: {e}")),
        }
    }
}
