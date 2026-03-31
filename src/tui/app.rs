use crate::config::Config;
use crate::types::{Message, StreamEvent, ToolUseBlock};
use std::collections::HashMap;

pub enum AppEvent {
    // Incoming from the engine thread
    EngineUpdate(StreamEvent),
    EngineTurn(u32),
    EngineError(String),
    EngineDone,
    // Add more granular events if needed (e.g. tools, cost)
    ToolStarted(ToolUseBlock),
    ToolFinished(String, String), // tool_id, result
    CostUpdate(f64, u32, u32, u32), // cost, in_tokens, out_tokens, context_pct
}

pub struct App {
    pub config: Config,
    pub messages: Vec<Message>,
    pub streaming_text: String,
    pub active_tools: HashMap<String, ToolUseBlock>,
    pub tool_results: HashMap<String, String>,
    pub current_turn: u32,
    pub is_running: bool,
    pub error: Option<String>,
    pub cost_usd: f64,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub context_pct: u32,
    pub user_input: tui_input::Input,
}

impl App {
    pub fn new(config: Config, initial_messages: Vec<Message>) -> Self {
        Self {
            config,
            messages: initial_messages,
            streaming_text: String::new(),
            active_tools: HashMap::new(),
            tool_results: HashMap::new(),
            current_turn: 1,
            is_running: true,
            error: None,
            cost_usd: 0.0,
            tokens_in: 0,
            tokens_out: 0,
            context_pct: 0,
            user_input: tui_input::Input::default(),
        }
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::EngineTurn(t) => self.current_turn = t,
            AppEvent::EngineError(e) => {
                self.error = Some(e);
                self.is_running = false;
            }
            AppEvent::EngineDone => self.is_running = false,
            AppEvent::EngineUpdate(ev) => match ev {
                StreamEvent::TextDelta(text) => self.streaming_text.push_str(&text),
                StreamEvent::ToolUseStart { index: _, id, name } => {
                    self.active_tools.insert(id.clone(), ToolUseBlock {
                        id,
                        name,
                        input: serde_json::Value::Null,
                    });
                }
                StreamEvent::ToolUseDelta { index: _, json_chunk: _ } => {
                    // We don't fully parse chunks here yet, just keep track
                }
                StreamEvent::ToolUseEnd { index: _ } => {}
                StreamEvent::Stop(_) => {
                    if !self.streaming_text.is_empty() {
                        self.messages.push(Message::assistant_text(self.streaming_text.clone()));
                        self.streaming_text.clear();
                    }
                }
                StreamEvent::Error(err) => {
                    self.error = Some(err);
                }
                _ => {}
            },
            AppEvent::ToolStarted(tool) => {
                self.active_tools.insert(tool.id.clone(), tool);
            }
            AppEvent::ToolFinished(id, result) => {
                self.active_tools.remove(&id);
                self.tool_results.insert(id, result);
            }
            AppEvent::CostUpdate(c, i, o, pct) => {
                self.cost_usd = c;
                self.tokens_in = i;
                self.tokens_out = o;
                self.context_pct = pct;
            }
        }
    }
}
