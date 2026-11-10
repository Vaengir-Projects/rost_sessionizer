//!
//! # Kill session handler
//!
//! This module handles the logic to kill current or all sessions.

use crate::{config, utils::tmux};
use anyhow::{Context, Result, bail};

/// Kills the session the user is currently attached to.
///
/// # Errors
///
/// Returns `io::Error` if any of the tmux commands fail.
pub fn kill_current_session() -> Result<()> {
    let default_session = config::default_session();
    let current_session = tmux::current_session().context("Error getting current session")?;

    tmux::tmux_switch_client(&default_session, None)
        .context("Error switching to default session")?;

    if current_session == default_session {
        tmux::tmux_display_message(&format!(
            "Can't kill the default session: '{default_session}'"
        ))
        .context("Error sending 'Can't kill the default session' notification")?;
    } else {
        tmux::tmux_kill_session(&current_session)
            .with_context(|| format!("Error killing current session: '{current_session}'"))?;
    }

    Ok(())
}

/// Kills all sessions the except the Default session.
///
/// # Errors
///
/// Returns `io::Error` if any of the tmux commands fail, or
/// `anyhow::Error` if parsing the existing sessions fails.
pub fn kill_all_sessions() -> Result<()> {
    let default_session = config::default_session();
    let mut sessions =
        tmux::existing_session_names().context("Error getting existing session names")?;
    sessions.retain(|s| !s.as_str().contains(default_session.as_str()));

    tmux::tmux_switch_client(&default_session, None)
        .context("Error switching to default session")?;

    let mut failed = Vec::new();
    for session in sessions {
        if let Err(e) = tmux::tmux_kill_session(&session) {
            failed.push(format!("{e:#}"));
        }
    }
    if !failed.is_empty() {
        bail!(
            "Failed to kill {} session(s):\n{}",
            failed.len(),
            failed.join("\n")
        );
    }

    Ok(())
}
