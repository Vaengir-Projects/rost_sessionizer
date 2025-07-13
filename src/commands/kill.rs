//!
//! # Kill session handler
//!
//! This module handles the logic to kill current or all sessions.

use crate::{DEFAULT_SESSION, utils};
use anyhow::{Context, Result};

pub fn kill_current_session() -> Result<()> {
    let current_session = utils::current_session().context("Error getting current session")?;

    utils::tmux_switch_client(DEFAULT_SESSION, None)
        .context("Error switching to default session")?;

    if current_session != DEFAULT_SESSION {
        utils::tmux_kill_session(&current_session)
            .with_context(|| format!("Error killing current session: '{current_session}'"))?;
    } else {
        utils::tmux_display_message(&format!(
            "Can't kill the default session: '{DEFAULT_SESSION}'"
        ))
        .context("Error sending 'Can't kill the default session' notification")?;
    }

    Ok(())
}

pub fn kill_all_sessions() -> Result<()> {
    let mut sessions =
        utils::existing_session_names().context("Error getting existing session names")?;
    sessions.retain(|s| !s.contains(DEFAULT_SESSION));

    utils::tmux_switch_client(DEFAULT_SESSION, None)
        .context("Error switching to default session")?;

    for session in sessions {
        utils::tmux_kill_session(&session)
            .with_context(|| format!("Error killing current session: '{session}'"))?;
    }

    Ok(())
}
