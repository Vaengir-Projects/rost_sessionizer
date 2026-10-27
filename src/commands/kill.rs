//!
//! # Kill session handler
//!
//! This module handles the logic to kill current or all sessions.

use crate::{config, utils::tmux};
use anyhow::{Context, Result};

/// Kills the session the user is currently attached to.
///
/// # Errors
///
/// Returns `io::Error` if any of the tmux commands fail.
pub fn kill_current_session() -> Result<()> {
    let current_session = tmux::current_session().context("Error getting current session")?;

    tmux::tmux_switch_client(&config::default_session(), None)
        .context("Error switching to default session")?;

    if current_session == config::default_session() {
        tmux::tmux_display_message(&format!(
            "Can't kill the default session: '{}'",
            config::default_session()
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
    let mut sessions =
        tmux::existing_session_names().context("Error getting existing session names")?;
    sessions.retain(|s| !s.contains(&config::default_session()));

    tmux::tmux_switch_client(&config::default_session(), None)
        .context("Error switching to default session")?;

    for session in sessions {
        tmux::tmux_kill_session(&session)
            .with_context(|| format!("Error killing current session: '{session}'"))?;
    }

    Ok(())
}
