use crate::config::{Config, Provider};
use crate::types::*;
use anyhow::Result;
use futures::StreamExt;
use serde_json::Value;

/// Multi-provider API client. Sends requests and parses SSE streams into
/// a unified Vec<StreamEvent> regardless of provider.
pub struct ApiClient {
    http: reqwest::Client,
    config: Config,
}

impl ApiClient {
    pub fn new(config: &Config) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .expect("failed to build HTTP client");
        Self { http, config: config.clone() }
    }

    /// Stream a message completion. Returns all events collected.
    pub async fn stream_message(
        &self,
        system: &str,
        messages: &[Message],
        tool_schemas: &[Value],
    ) -> Result<Vec<StreamEvent>> {
        match self.config.provider {
            Provider::Claude => self.stream_claude(system, messages, tool_schemas).await,
            Provider::OpenAI | Provider::Gemini | Provider::Custom => {
                self.stream_openai(system, messages, tool_schemas).await
            }
        }
    }



    async fn stream_claude(
        &self,
        system: &str,
        messages: &[Message],
        tool_schemas: &[Value],
    ) -> Result<Vec<StreamEvent>> {
        let api_messages = messages
            .iter()
            .map(|m| msg_to_claude(m))
            .collect::<Vec<_>>();

        let mut body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "stream": true,
            "system": system,
            "messages": api_messages,
        });

        if !tool_schemas.is_empty() {
            body["tools"] = Value::Array(tool_schemas.to_vec());
        }

        let url = format!("{}/v1/messages", self.config.base_url);

        let resp = self
            .http
            .post(&url)
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .body(serde_json::to_string(&body)?)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Claude API error {status}: {text}");
        }

        parse_sse_stream(resp, parse_claude_event).await
    }



    async fn stream_openai(
        &self,
        system: &str,
        messages: &[Message],
        tool_schemas: &[Value],
    ) -> Result<Vec<StreamEvent>> {
        let mut api_messages = vec![serde_json::json!({
            "role": "system",
            "content": system,
        })];

        for m in messages {
            api_messages.push(msg_to_openai(m));
        }

        let mut body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "stream": true,
            "stream_options": { "include_usage": true },
            "messages": api_messages,
        });

        if !tool_schemas.is_empty() {
            body["tools"] = Value::Array(tool_schemas.to_vec());
        }

        let url = format!("{}/v1/chat/completions", self.config.base_url);

        let mut req = self
            .http
            .post(&url)
            .header("content-type", "application/json");

        if !self.config.api_key.is_empty() {
            req = req.header("authorization", format!("Bearer {}", self.config.api_key));
        }

        let resp = req.body(serde_json::to_string(&body)?).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI API error {status}: {text}");
        }

        parse_sse_stream(resp, parse_openai_event).await
    }
}



async fn parse_sse_stream(
    resp: reqwest::Response,
    parse_fn: fn(&str) -> Vec<StreamEvent>,
) -> Result<Vec<StreamEvent>> {
    let mut events = Vec::new();
    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim_end_matches('\r').to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    return Ok(events);
                }
                let mut parsed_events = parse_fn(data);
                events.append(&mut parsed_events);
            }
        }
    }

    Ok(events)
}

fn parse_claude_event(data: &str) -> Vec<StreamEvent> {
    let mut events = Vec::new();

    let v: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return events,
    };

    let event_type = match v.get("type").and_then(|t| t.as_str()) {
        Some(t) => t,
        None => return events,
    };

    match event_type {
        "content_block_start" => {
            if let Some(index) = v.get("index").and_then(|i| i.as_u64()).map(|i| i as usize) {
                if let Some(block) = v.get("content_block") {
                    if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        if let (Some(id), Some(name)) = (
                            block.get("id").and_then(|i| i.as_str()),
                            block.get("name").and_then(|n| n.as_str()),
                        ) {
                            events.push(StreamEvent::ToolUseStart {
                                index,
                                id: id.to_string(),
                                name: name.to_string(),
                            });
                        }
                    }
                }
            }
        }
        "content_block_delta" => {
            if let Some(index) = v.get("index").and_then(|i| i.as_u64()).map(|i| i as usize) {
                if let Some(delta) = v.get("delta") {
                    if let Some(delta_type) = delta.get("type").and_then(|t| t.as_str()) {
                        match delta_type {
                            "text_delta" => {
                                if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                    events.push(StreamEvent::TextDelta(text.to_string()));
                                }
                            }
                            "thinking_delta" => {
                                if let Some(text) = delta.get("thinking").and_then(|t| t.as_str()) {
                                    events.push(StreamEvent::ThinkingDelta(text.to_string()));
                                }
                            }
                            "input_json_delta" => {
                                if let Some(json) = delta.get("partial_json").and_then(|j| j.as_str()) {
                                    events.push(StreamEvent::ToolUseDelta {
                                        index,
                                        json_chunk: json.to_string(),
                                    });
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        "content_block_stop" => {
            if let Some(index) = v.get("index").and_then(|i| i.as_u64()).map(|i| i as usize) {
                events.push(StreamEvent::ToolUseEnd { index });
            }
        }
        "message_start" => {
            if let Some(usage) = v.pointer("/message/usage") {
                events.push(StreamEvent::Usage(parse_usage_claude(usage)));
            }
        }
        "message_delta" => {
            if let Some(stop) = v.pointer("/delta/stop_reason").and_then(|s| s.as_str()) {
                events.push(StreamEvent::Stop(StopReason::from_str_loose(stop)));
            }
            if let Some(usage) = v.get("usage") {
                events.push(StreamEvent::Usage(parse_usage_claude(usage)));
            }
        }
        "error" => {
            let msg = v
                .pointer("/error/message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown API error")
                .to_string();
            events.push(StreamEvent::Error(msg));
        }
        _ => {}
    }

    events
}

fn parse_usage_claude(v: &Value) -> Usage {
    Usage {
        input_tokens: v.get("input_tokens").and_then(|x| x.as_u64()).unwrap_or(0),
        output_tokens: v.get("output_tokens").and_then(|x| x.as_u64()).unwrap_or(0),
        cache_read_input_tokens: v
            .get("cache_read_input_tokens")
            .and_then(|x| x.as_u64())
            .unwrap_or(0),
        cache_creation_input_tokens: v
            .get("cache_creation_input_tokens")
            .and_then(|x| x.as_u64())
            .unwrap_or(0),
    }
}



fn parse_openai_event(data: &str) -> Vec<StreamEvent> {
    let mut events = Vec::new();

    let v: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return events,
    };

    // Usage (sent with stream_options.include_usage)
    if let Some(usage) = v.get("usage") {
        if !usage.is_null() {
            events.push(StreamEvent::Usage(Usage {
                input_tokens: usage
                    .get("prompt_tokens")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0),
                output_tokens: usage
                    .get("completion_tokens")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0),
                ..Default::default()
            }));
        }
    }

    let choice = match v.get("choices").and_then(|c| c.as_array()).and_then(|a| a.first()) {
        Some(c) => c,
        None => return events,
    };
    
    let delta = match choice.get("delta") {
        Some(d) => d,
        None => return events,
    };

    // Text content
    if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
        if !content.is_empty() {
            events.push(StreamEvent::TextDelta(content.to_string()));
        }
    }

    // Tool calls
    if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
        for tc in tool_calls {
            let index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;

            // Start event (has id + function.name)
            if let Some(id) = tc.get("id").and_then(|i| i.as_str()) {
                let name = tc
                    .pointer("/function/name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                events.push(StreamEvent::ToolUseStart {
                    index,
                    id: id.to_string(),
                    name,
                });
            }

            // Delta event (has function.arguments chunk)
            if let Some(args) = tc.pointer("/function/arguments").and_then(|a| a.as_str()) {
                if !args.is_empty() {
                    events.push(StreamEvent::ToolUseDelta {
                        index,
                        json_chunk: args.to_string(),
                    });
                }
            }
        }
    }

    // Finish reason - handle this LAST so that text+tool in same chunk aren't dropped
    if let Some(reason) = choice.get("finish_reason") {
        if !reason.is_null() {
            if let Some(r) = reason.as_str() {
                events.push(StreamEvent::Stop(StopReason::from_str_loose(r)));
            }
        }
    }

    events
}



fn msg_to_claude(m: &Message) -> Value {
    match &m.content {
        MessageContent::Text(text) => {
            serde_json::json!({
                "role": role_str(&m.role),
                "content": text,
            })
        }
        MessageContent::Blocks(blocks) => {
            let content_blocks: Vec<Value> = blocks
                .iter()
                .map(|b| match b {
                    ContentBlock::Text { text } => serde_json::json!({
                        "type": "text",
                        "text": text,
                    }),
                    ContentBlock::ToolUse(tu) => serde_json::json!({
                        "type": "tool_use",
                        "id": tu.id,
                        "name": tu.name,
                        "input": tu.input,
                    }),
                    ContentBlock::ToolResult(tr) => serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": tr.tool_use_id,
                        "content": tr.content,
                        "is_error": tr.is_error,
                    }),
                    ContentBlock::Thinking { thinking } => serde_json::json!({
                        "type": "thinking",
                        "thinking": thinking,
                    }),
                })
                .collect();
            serde_json::json!({
                "role": role_str(&m.role),
                "content": content_blocks,
            })
        }
    }
}

fn msg_to_openai(m: &Message) -> Value {
    match &m.content {
        MessageContent::Text(text) => {
            serde_json::json!({
                "role": openai_role(&m.role),
                "content": text,
            })
        }
        MessageContent::Blocks(blocks) => {
            // Check if this is a tool result message
            let tool_results: Vec<&ToolResultBlock> = blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolResult(tr) => Some(tr),
                    _ => None,
                })
                .collect();

            if tool_results.len() == 1 {
                let tr = tool_results[0];
                return serde_json::json!({
                    "role": "tool",
                    "tool_call_id": tr.tool_use_id,
                    "content": tr.content,
                });
            }

            // Check if this is an assistant message with tool use
            let tool_uses: Vec<&ToolUseBlock> = blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolUse(tu) => Some(tu),
                    _ => None,
                })
                .collect();

            let text_content: String = blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("");

            if !tool_uses.is_empty() {
                let tool_calls: Vec<Value> = tool_uses
                    .iter()
                    .map(|tu| {
                        serde_json::json!({
                            "id": tu.id,
                            "type": "function",
                            "function": {
                                "name": tu.name,
                                "arguments": serde_json::to_string(&tu.input).unwrap_or_default(),
                            }
                        })
                    })
                    .collect();

                let mut msg = serde_json::json!({
                    "role": "assistant",
                    "tool_calls": tool_calls,
                });
                if !text_content.is_empty() {
                    msg["content"] = Value::String(text_content);
                }
                return msg;
            }

            // For multiple tool results, we need separate messages (OpenAI constraint)
            // Return just the first one; engine handles multi-result splitting
            if !tool_results.is_empty() {
                let tr = tool_results[0];
                return serde_json::json!({
                    "role": "tool",
                    "tool_call_id": tr.tool_use_id,
                    "content": tr.content,
                });
            }

            serde_json::json!({
                "role": openai_role(&m.role),
                "content": text_content,
            })
        }
    }
}

fn role_str(role: &Role) -> &'static str {
    match role {
        Role::User | Role::Tool => "user",
        Role::Assistant => "assistant",
        Role::System => "system",
    }
}

fn openai_role(role: &Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::System => "system",
        Role::Tool => "tool",
    }
}
