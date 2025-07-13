//!
//! # Utils section
//!
//! This module provides functions that are used internally.

use anyhow::{Context, Result, anyhow};
use std::process::Command;

pub(crate) fn tmux_command_without_output(args: &[&str]) -> Result<()> {
    Command::new("tmux")
        .args(args)
        .status()
        .with_context(|| format!("Error running tmux command `tmux {args:#?}"))?;

    Ok(())
}

pub(crate) fn tmux_switch_client(target_session: &str, target_window: Option<usize>) -> Result<()> {
    tmux_command_without_output(&[
        "switch-client",
        "-t",
        &format!("{target_session}:{}", target_window.unwrap_or(1)),
    ])
    .with_context(|| format!("Error switching tmux client to '{target_session}'"))?;

    Ok(())
}

pub(crate) fn tmux_kill_session(target_session: &str) -> Result<()> {
    tmux_command_without_output(&["kill-session", "-t", &target_session])
        .with_context(|| format!("Error killing session '{target_session}'"))?;

    Ok(())
}

pub(crate) fn tmux_display_message(message: &str) -> Result<()> {
    tmux_command_without_output(&["display-message", message]).context("Error sending notification")
}

pub(crate) fn tmux_session_exisits(target_session: &str) -> Result<bool> {
    Command::new("tmux")
        .args(["has-session", "-t", target_session])
        .status()
        .map(|status| status.success())
        .context("Error checking if session exists")
}

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
