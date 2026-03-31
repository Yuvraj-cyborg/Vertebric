use crate::api::ApiClient;
use crate::config::{Config, Provider};
use crate::cost::CostTracker;
use crate::session;
use crate::tools::{ToolContext, ToolRegistry, ToolResult};
use crate::types::*;
use colored::Colorize;
use std::path::PathBuf;

/// The agentic query loop. Sends prompts to the API, executes tools,
/// and loops until the model stops or limits are hit.
///
/// Ported from QueryEngine.ts + query.ts
pub struct Engine {
    api: ApiClient,
    tools: ToolRegistry,
    cost: CostTracker,
    messages: Vec<Message>,
    system_prompt: String,
    config: Config,
    session_dir: PathBuf,
    turn: u32,
}

pub enum EngineResult {
    Done(String),          // final text response
    MaxTurns,              // hit turn limit
    MaxBudget,             // hit budget limit
    Error(String),         // unrecoverable error
}

impl Engine {
    pub fn new(config: Config, system_prompt: String) -> anyhow::Result<Self> {
        let api = ApiClient::new(&config);
        let tools = ToolRegistry::new(config.cwd.clone());
        let cost = CostTracker::new(&config.model);
        let session_dir = session::create_session()?;

        Ok(Self {
            api,
            tools,
            cost,
            messages: Vec::new(),
            system_prompt,
            config,
            session_dir,
            turn: 0,
        })
    }

    /// Run the agentic loop for a single user prompt.
    /// Conversation history is preserved across calls; turn counter resets per prompt.
    pub async fn run(&mut self, prompt: &str) -> EngineResult {
        self.turn = 0;
        let user_msg = Message::user(prompt);
        self.messages.push(user_msg.clone());
        let _ = session::append_message(&self.session_dir, &user_msg);

        self.query_loop().await
    }

    async fn query_loop(&mut self) -> EngineResult {
        let tool_schemas = match self.config.provider {
            Provider::Claude => self.tools.schemas(),
            Provider::OpenAI | Provider::Gemini | Provider::Custom => self.tools.schemas_openai(),
        };

        loop {
            self.turn += 1;


            if let Some(max) = self.config.max_turns {
                if self.turn > max {
                    eprintln!("{}", format!("⚠ Max turns ({max}) reached").yellow());
                    return EngineResult::MaxTurns;
                }
            }


            if let Some(max_budget) = self.config.max_budget_usd {
                if self.cost.total_cost_usd >= max_budget {
                    eprintln!(
                        "{}",
                        format!("⚠ Budget limit (${max_budget:.2}) reached").yellow()
                    );
                    return EngineResult::MaxBudget;
                }
            }

            if self.config.verbose {
                eprintln!(
                    "{}",
                    format!(
                        "─── Turn {} | {} ───",
                        self.turn,
                        self.cost.format_cost()
                    )
                    .dimmed()
                );
            }


            let events = match self
                .api
                .stream_message(&self.system_prompt, &self.messages, &tool_schemas)
                .await
            {
                Ok(e) => e,
                Err(e) => {
                    return EngineResult::Error(format!("API error: {e}"));
                }
            };


            let mut text_parts: Vec<String> = Vec::new();
            let mut thinking_parts: Vec<String> = Vec::new();

            // Tool use accumulation: index → (id, name, json_chunks)
            let mut tool_uses: Vec<(String, String, Vec<String>)> = Vec::new();
            let mut usage = Usage::default();
            let mut stop_reason = StopReason::EndTurn;

            for event in events {
                match event {
                    StreamEvent::TextDelta(text) => {
                        print!("{text}");
                        text_parts.push(text);
                    }
                    StreamEvent::ThinkingDelta(text) => {
                        thinking_parts.push(text);
                    }
                    StreamEvent::ToolUseStart { index, id, name } => {
                        // Ensure vec is big enough
                        while tool_uses.len() <= index {
                            tool_uses.push((String::new(), String::new(), Vec::new()));
                        }
                        tool_uses[index].0 = id;
                        tool_uses[index].1 = name;
                    }
                    StreamEvent::ToolUseDelta { index, json_chunk } => {
                        if index < tool_uses.len() {
                            tool_uses[index].2.push(json_chunk);
                        }
                    }
                    StreamEvent::ToolUseEnd { .. } => {}
                    StreamEvent::Usage(u) => {
                        usage = u;
                    }
                    StreamEvent::Stop(s) => {
                        stop_reason = s;
                    }
                    StreamEvent::Error(e) => {
                        return EngineResult::Error(format!("Stream error: {e}"));
                    }
                }
            }

            // Track cost
            let turn_cost = self.cost.add(&usage);
            if self.config.verbose {
                eprintln!(
                    "{}",
                    format!(
                        "  tokens: {} in / {} out | cost: ${:.4}",
                        usage.input_tokens, usage.output_tokens, turn_cost
                    )
                    .dimmed()
                );
            }


            let mut content_blocks: Vec<ContentBlock> = Vec::new();

            if !thinking_parts.is_empty() {
                content_blocks.push(ContentBlock::Thinking {
                    thinking: thinking_parts.join(""),
                });
            }

            let full_text = text_parts.join("");
            if !full_text.is_empty() {
                content_blocks.push(ContentBlock::Text {
                    text: full_text.clone(),
                });
            }

            // Parse tool use blocks
            let mut parsed_tool_uses: Vec<ToolUseBlock> = Vec::new();
            for (id, name, json_chunks) in &tool_uses {
                if id.is_empty() {
                    continue;
                }
                let json_str = json_chunks.join("");
                let input: serde_json::Value = serde_json::from_str(&json_str)
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

                let tu = ToolUseBlock {
                    id: id.clone(),
                    name: name.clone(),
                    input,
                };
                content_blocks.push(ContentBlock::ToolUse(tu.clone()));
                parsed_tool_uses.push(tu);
            }

            let assistant_msg = Message::assistant_blocks(content_blocks);
            self.messages.push(assistant_msg.clone());
            let _ = session::append_message(&self.session_dir, &assistant_msg);


            if !parsed_tool_uses.is_empty() {
                // Print newline after text if there was any
                if !full_text.is_empty() {
                    println!();
                }

                let tool_ctx = ToolContext {
                    cwd: self.config.cwd.clone(),
                };

                let mut results: Vec<ToolResultBlock> = Vec::new();

                for tu in &parsed_tool_uses {
                    eprintln!(
                        "{}",
                        format!("⚡ {}({})", tu.name, truncate_json(&tu.input, 80)).cyan()
                    );

                    let result: ToolResult =
                        self.tools.execute(&tu.name, tu.input.clone(), &tool_ctx).await;

                    if result.is_error {
                        eprintln!(
                            "{}",
                            format!("  ✗ {}", truncate_str(&result.content, 120)).red()
                        );
                    } else if self.config.verbose {
                        eprintln!(
                            "{}",
                            format!("  ✓ {}", truncate_str(&result.content, 120)).green()
                        );
                    }

                    results.push(ToolResultBlock {
                        tool_use_id: tu.id.clone(),
                        content: result.content,
                        is_error: result.is_error,
                    });
                }

                // For OpenAI, each tool result is a separate message
                match self.config.provider {
                    Provider::OpenAI | Provider::Gemini | Provider::Custom => {
                        for tr in &results {
                            let msg = Message {
                                role: Role::Tool,
                                content: MessageContent::Blocks(vec![ContentBlock::ToolResult(
                                    tr.clone(),
                                )]),
                                uuid: Some(uuid::Uuid::new_v4().to_string()),
                            };
                            self.messages.push(msg.clone());
                            let _ = session::append_message(&self.session_dir, &msg);
                        }
                    }
                    Provider::Claude => {
                        let msg = Message::tool_results(results);
                        self.messages.push(msg.clone());
                        let _ = session::append_message(&self.session_dir, &msg);
                    }
                }

                // Continue the loop — send tool results back to the API
                continue;
            }


            match stop_reason {
                StopReason::EndTurn => {
                    println!(); // final newline after streamed text
                    return EngineResult::Done(full_text);
                }
                StopReason::MaxTokens => {
                    // Model hit output limit — could continue but for now just return
                    eprintln!(
                        "{}",
                        "⚠ Response truncated (max output tokens)".yellow()
                    );
                    println!();
                    return EngineResult::Done(full_text);
                }
                _ => {
                    println!();
                    return EngineResult::Done(full_text);
                }
            }
        }
    }

    pub fn cost_summary(&self) -> String {
        self.cost.format_summary()
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    let s = s.replace('\n', " ");
    if s.len() > max {
        format!("{}…", &s[..max])
    } else {
        s
    }
}

fn truncate_json(v: &serde_json::Value, max: usize) -> String {
    let s = serde_json::to_string(v).unwrap_or_default();
    truncate_str(&s, max)
}
