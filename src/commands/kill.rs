//!
//! # Kill session handler
//!
//! This module handles the logic to kill current or all sessions.

use crate::{DEFAULT_SESSION, utils};
use anyhow::{Context, Result};
use std::process::Command;

pub fn kill_current_session() -> Result<()> {
    let current_session = utils::current_session().context("Error getting current session")?;

    Command::new("tmux")
        .args(["switch-client", "-t", &format!("{DEFAULT_SESSION}:1")])
        .status()
        .context("Error switching to default session")?;

    if current_session != DEFAULT_SESSION {
        Command::new("tmux")
            .args(["kill-session", "-t", &current_session])
            .status()
            .with_context(|| format!("Error killing current session: '{current_session}'"))?;
    } else {
        Command::new("tmux")
            .args([
                "display-message",
                &format!("Can't kill the default session: '{DEFAULT_SESSION}'"),
            ])
            .status()
            .context("Error sending notification")?;
    }

    Ok(())
}

pub fn kill_all_sessions() -> Result<()> {
    let mut sessions =
        utils::existing_session_names().context("Error getting existing session names")?;
    sessions.retain(|s| !s.contains(DEFAULT_SESSION));

    Command::new("tmux")
        .args(["switch-client", "-t", &format!("{DEFAULT_SESSION}:1")])
        .status()
        .context("Error switching to default session")?;

    for session in sessions {
        Command::new("tmux")
            .args(["kill-session", "-t", &session])
            .status()
            .with_context(|| format!("Error killing current session: '{session}'"))?;
    }

    Ok(())
}
