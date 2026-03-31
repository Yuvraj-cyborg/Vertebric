use super::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::Value;

pub struct WebFetchTool;

const MAX_BODY: usize = 50_000;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &'static str { "web_fetch" }

    fn description(&self) -> &'static str {
        "Fetch the content of a URL. Returns the response body as text. Useful for reading \
         documentation, APIs, or web pages. Large responses are truncated."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "The URL to fetch" }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let url = match input.get("url").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => return ToolResult::err("Missing 'url'"),
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build();

        let client = match client {
            Ok(c) => c,
            Err(e) => return ToolResult::err(format!("HTTP client error: {e}")),
        };

        match client.get(url).send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                match resp.text().await {
                    Ok(mut body) => {
                        if body.len() > MAX_BODY {
                            body.truncate(MAX_BODY);
                            body.push_str("\n... (response truncated)");
                        }
                        if status >= 400 {
                            ToolResult::err(format!("HTTP {status}\n{body}"))
                        } else {
                            ToolResult::ok(body)
                        }
                    }
                    Err(e) => ToolResult::err(format!("Failed to read response: {e}")),
                }
            }
            Err(e) => ToolResult::err(format!("Request failed: {e}")),
        }
    }
}
