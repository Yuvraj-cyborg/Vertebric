use serde::{Deserialize, Serialize};
use serde_json::Value;



#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
    Tool, // OpenAI tool result role
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl MessageContent {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            Self::Blocks(blocks) => blocks.iter().find_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            }),
        }
    }

    pub fn text_concat(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(""),
        }
    }

    pub fn tool_uses(&self) -> Vec<&ToolUseBlock> {
        match self {
            Self::Text(_) => vec![],
            Self::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolUse(tu) => Some(tu),
                    _ => None,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse(ToolUseBlock),
    #[serde(rename = "tool_result")]
    ToolResult(ToolResultBlock),
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseBlock {
    pub id: String,
    pub name: String,
    pub input: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultBlock {
    pub tool_use_id: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub is_error: bool,
}



#[derive(Debug, Clone)]
pub enum StreamEvent {
    TextDelta(String),
    ThinkingDelta(String),
    ToolUseStart {
        index: usize,
        id: String,
        name: String,
    },
    ToolUseDelta {
        index: usize,
        json_chunk: String,
    },
    ToolUseEnd {
        index: usize,
    },
    Usage(Usage),
    Stop(StopReason),
    Error(String),
}



#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
}

impl Usage {
    pub fn accumulate(&mut self, other: &Usage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_input_tokens += other.cache_read_input_tokens;
        self.cache_creation_input_tokens += other.cache_creation_input_tokens;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
    Unknown(String),
}

impl StopReason {
    pub fn from_str_loose(s: &str) -> Self {
        match s {
            "end_turn" | "stop" => Self::EndTurn,
            "tool_use" | "tool_calls" => Self::ToolUse,
            "max_tokens" | "length" => Self::MaxTokens,
            "stop_sequence" => Self::StopSequence,
            other => Self::Unknown(other.to_string()),
        }
    }
}



impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Text(text.into()),
            uuid: Some(uuid::Uuid::new_v4().to_string()),
        }
    }

    pub fn assistant_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: MessageContent::Blocks(vec![ContentBlock::Text {
                text: text.into(),
            }]),
            uuid: Some(uuid::Uuid::new_v4().to_string()),
        }
    }

    pub fn assistant_blocks(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: Role::Assistant,
            content: MessageContent::Blocks(blocks),
            uuid: Some(uuid::Uuid::new_v4().to_string()),
        }
    }

    pub fn tool_results(results: Vec<ToolResultBlock>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Blocks(
                results.into_iter().map(ContentBlock::ToolResult).collect(),
            ),
            uuid: Some(uuid::Uuid::new_v4().to_string()),
        }
    }

    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: MessageContent::Text(text.into()),
            uuid: None,
        }
    }
}

pub fn rough_token_estimate(text: &str) -> u64 {
    (text.len() as u64) / 4
}

pub fn rough_message_tokens(messages: &[Message]) -> u64 {
    messages
        .iter()
        .map(|m| rough_token_estimate(&m.content.text_concat()))
        .sum()
}
