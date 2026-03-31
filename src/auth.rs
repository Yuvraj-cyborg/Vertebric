use crate::config::Provider;
use anyhow::{Context, Result};
use dialoguer::Password;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Default, Serialize, Deserialize)]
struct Credentials {
    keys: HashMap<String, String>,
}

fn credentials_path() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "vertebric", "vertebric")
        .context("Could not determine user home directory")?;
    let dir = proj_dirs.config_dir();
    std::fs::create_dir_all(dir)?;
    Ok(dir.join("credentials.json"))
}

fn load_credentials() -> Credentials {
    if let Ok(path) = credentials_path() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(creds) = serde_json::from_str(&content) {
                return creds;
            }
        }
    }
    Credentials::default()
}

fn save_credentials(creds: &Credentials) -> Result<()> {
    let path = credentials_path()?;
    let content = serde_json::to_string_pretty(creds)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Retrieves an API key. 
/// 1. Checks environment variable.
/// 2. Checks local config `credentials.json`.
/// 3. If missing, prompts the user via a hidden input terminal prompt and saves it.
pub fn get_or_prompt_api_key(provider: &Provider) -> Result<String> {
    let env_var = provider.api_key_env();
    
    // 1. Try Environment Variable (takes precedence)
    if let Ok(key) = std::env::var(env_var) {
        if !key.trim().is_empty() {
            return Ok(key);
        }
    }

    // Custom endpoints without a required key can just use an empty string or "dummy"
    // But we'll still prompt if the user hasn't set one, unless we specifically bypass.
    // For now, prompt for everything unless they hit enter for an empty key on Custom.

    // 2. Try Local Config
    let mut creds = load_credentials();
    let provider_name = match provider {
        Provider::Claude => "claude",
        Provider::OpenAI => "openai",
        Provider::Gemini => "gemini",
        Provider::Custom => "custom",
    };

    if let Some(key) = creds.keys.get(provider_name) {
        if !key.trim().is_empty() {
            return Ok(key.clone());
        }
    }

    // 3. Interactive Prompt
    eprintln!();
    eprintln!("No API key found for {}.", provider.provider_display());
    eprintln!(
        "Set {} in your environment, or enter it now.\n",
        provider.api_key_env()
    );

    let prompt_msg = format!("{} API key", provider.provider_display());
    let new_key = Password::new()
        .with_prompt(&prompt_msg)
        .interact()?;

    creds.keys.insert(provider_name.to_string(), new_key.clone());
    save_credentials(&creds)?;

    eprintln!("API key saved.\n");

    Ok(new_key)
}
