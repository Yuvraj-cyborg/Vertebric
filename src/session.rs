use crate::types::Message;
use anyhow::Result;
use std::path::PathBuf;

/// Append a message to the session JSONL transcript
pub fn append_message(session_dir: &PathBuf, message: &Message) -> Result<()> {
    use std::io::Write;
    let path = session_dir.join("transcript.jsonl");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    let json = serde_json::to_string(message)?;
    writeln!(file, "{json}")?;
    Ok(())
}

/// Load all messages from a session transcript
pub fn load_session(session_dir: &PathBuf) -> Result<Vec<Message>> {
    let path = session_dir.join("transcript.jsonl");
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = std::fs::read_to_string(&path)?;
    let mut messages = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Message>(line) {
            Ok(m) => messages.push(m),
            Err(_) => continue, // skip malformed lines
        }
    }
    Ok(messages)
}

/// Create a new session directory and return its path
pub fn create_session() -> Result<PathBuf> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let dir = crate::context::sessions_dir().join(&session_id);
    std::fs::create_dir_all(&dir)?;

    // Write session metadata
    let meta = serde_json::json!({
        "id": session_id,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "cwd": std::env::current_dir()?.to_string_lossy(),
    });
    std::fs::write(
        dir.join("metadata.json"),
        serde_json::to_string_pretty(&meta)?,
    )?;

    Ok(dir)
}
