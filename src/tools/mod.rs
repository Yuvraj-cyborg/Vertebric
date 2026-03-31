pub mod bash;
pub mod file_edit;
pub mod file_read;
pub mod file_write;
pub mod glob_tool;
pub mod grep;
pub mod web_fetch;

use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;



#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> Value;
    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult;
}

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub cwd: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
}

impl ToolResult {
    pub fn ok(content: impl Into<String>) -> Self {
        Self { content: content.into(), is_error: false }
    }
    pub fn err(content: impl Into<String>) -> Self {
        Self { content: content.into(), is_error: true }
    }
}



pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new(cwd: PathBuf) -> Self {
        let _ = cwd; // tools get cwd via ToolContext at execution time
        let tools: Vec<Box<dyn Tool>> = vec![
            Box::new(bash::BashTool),
            Box::new(file_read::FileReadTool),
            Box::new(file_write::FileWriteTool),
            Box::new(file_edit::FileEditTool),
            Box::new(grep::GrepTool),
            Box::new(glob_tool::GlobTool),
            Box::new(web_fetch::WebFetchTool),
        ];
        Self { tools }
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter().find(|t| t.name() == name).map(|t| t.as_ref())
    }

    pub async fn execute(&self, name: &str, input: Value, ctx: &ToolContext) -> ToolResult {
        match self.get(name) {
            Some(tool) => tool.execute(input, ctx).await,
            None => ToolResult::err(format!("Unknown tool: {name}")),
        }
    }

    /// Returns tool schemas in Claude format (name + description + input_schema).
    /// OpenAI adapter wraps these in { type: "function", function: { ... } }.
    pub fn schemas(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name(),
                    "description": t.description(),
                    "input_schema": t.input_schema(),
                })
            })
            .collect()
    }

    /// Returns tool schemas in OpenAI format
    pub fn schemas_openai(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name(),
                        "description": t.description(),
                        "parameters": t.input_schema(),
                    }
                })
            })
            .collect()
    }
}
