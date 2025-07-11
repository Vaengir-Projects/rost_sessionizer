//!
//! # Utils section
//!
//! This module provides functions that are used internally.

use anyhow::{Context, Result, anyhow};
use std::process::Command;

pub(crate) fn existing_sessions() -> Result<String> {
    let existing_sessions = Command::new("tmux")
        .arg("ls")
        .output()
        .context("Error listing existing tmux sessions")?
        .stdout;

    Ok(String::from_utf8_lossy(&existing_sessions).to_string())
}

pub(crate) fn current_session() -> Result<String> {
    let existing_sessions = existing_sessions().context("Error getting existing sessions")?;
    let current_session = existing_sessions
        .lines()
        .map(|s| s.to_string())
        .find(|s| s.contains("attached"))
        .context("No current session available")?;

    match current_session.find(':') {
        Some(i) => Ok(current_session[..i].to_string()),
        None => Err(anyhow!("Error parsing current session name")),
    }
}
