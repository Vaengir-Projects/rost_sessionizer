//!
//! # Utils section
//!
//! This module provides functions that are used internally.

use anyhow::{Context, Result, anyhow};
use std::process::Command;

pub(crate) fn existing_sessions() -> Result<Vec<String>> {
    let existing_sessions = Command::new("tmux")
        .arg("ls")
        .output()
        .context("Error listing existing tmux sessions")?
        .stdout;
    let existing_sessions = String::from_utf8_lossy(&existing_sessions).to_string();

    Ok(existing_sessions
        .lines()
        .map(|s| s.to_string())
        .collect::<Vec<String>>())
}

pub(crate) fn current_session() -> Result<String> {
    let existing_sessions = existing_sessions().context("Error getting existing sessions")?;
    let current_session = existing_sessions
        .iter()
        .find(|s| s.contains("attached"))
        .context("No current session available")?;

    match current_session.find(':') {
        Some(i) => Ok(current_session[..i].to_string()),
        None => Err(anyhow!("Error parsing current session name")),
    }
}

pub(crate) fn existing_session_names() -> Result<Vec<String>> {
    let existing_sessions = existing_sessions().context("Error getting existing sessions")?;

    Ok(existing_sessions
        .iter()
        .map(|s| {
            s.split_once(':')
                .context("Error parsing session names")
                .unwrap()
                .0
                .to_string()
        })
        .collect())
}
