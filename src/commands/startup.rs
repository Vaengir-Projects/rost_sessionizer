//!
//! # Start up handler
//!
//! This module handles the logic to start up the default session.

use crate::{
    config,
    utils::{tmux, types::session_name::SessionName},
};
use anyhow::{Context, Result};
use std::env;

/// Allows user to select the session they want to open.
///
/// # Errors
///
/// Returns `io::Error` if any of the tmux commands fail.
pub fn startup() -> Result<()> {
    let default_session = config::default_session();
    match env::var("TMUX") {
        Ok(val) if !val.is_empty() => {
            let default_session_exists = tmux::tmux_session_exisits(&default_session)
                .with_context(|| {
                    format!("Error checking if default session '{default_session}' exists")
                })?;
            if default_session_exists {
                tmux::tmux_display_message(&format!(
                    "The default session '{default_session}' is already running"
                ))
                .context("Error sending 'Default session already running' notification")?;
            } else {
                create_default_session(&default_session).with_context(|| {
                    format!("Error creating default session '{default_session}'")
                })?;
                tmux::tmux_switch_client(&default_session, Some(1)).with_context(|| {
                    format!(
                        "Error switching to first window of default session '{default_session}'"
                    )
                })?;
            }
        }
        _ => {
            create_default_session(&default_session)
                .with_context(|| format!("Error creating default session '{default_session}'"))?;
            tmux::tmux_command_without_output(&[
                "attach-session",
                "-t",
                &format!("{default_session}:1"),
            ])
            .with_context(|| format!("Error attaching to default session '{default_session}'"))?;
        }
    }

    Ok(())
}

fn create_default_session(default_session: &SessionName) -> Result<()> {
    let default_session_exists = tmux::tmux_session_exisits(default_session)
        .with_context(|| format!("Error checking if default session '{default_session}' exists"))?;
    // If default session already exists just exit early
    if default_session_exists {
        return Ok(());
    }

    let home = env::var("HOME").context("Error getting $HOME")?;
    tmux::tmux_command_without_output(&[
        "new-session",
        "-ds",
        default_session.as_str(),
        "-c",
        &home,
    ])
    .with_context(|| format!("Error creating default tmux session '{default_session}'"))?;
    tmux::tmux_command_without_output(&[
        "new-window",
        "-t",
        &format!("{default_session}:2"),
        "-c",
        &home,
    ])
    .with_context(|| {
        format!("Error creating second window for default session '{default_session}'")
    })?;

    Ok(())
}
