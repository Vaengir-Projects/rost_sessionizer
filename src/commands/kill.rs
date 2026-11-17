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
    let default_session = config::default_session();
    // Must run before switching: afterwards the client is in the default session.
    let current_session = tmux::current_session().context("Error getting current session")?;

    tmux::tmux_switch_client(&default_session).context("Error switching to default session")?;

    if current_session == default_session {
        tmux::tmux_display_message(&format!(
            "Can't kill the default session: '{default_session}'"
        ))
        .context("Error sending 'Can't kill the default session' notification")?;
    } else {
        tmux::tmux_kill_session(&current_session).context("Error killing current session")?;
    }

    Ok(())
}

/// Kills all sessions except the default session.
///
/// # Errors
///
/// Returns `io::Error` if any of the tmux commands fail.
pub fn kill_all_sessions() -> Result<()> {
    let default_session = config::default_session();

    tmux::tmux_switch_client(&default_session).context("Error switching to default session")?;
    tmux::tmux_kill_other_sessions(&default_session).context("Error killing all other sessions")?;

    Ok(())
}
