use crate::{DEFAULT_SESSION, utils};
use anyhow::{Context, Result};
use std::env;

/// # Errors
///
/// Will return `Err` if any of the tmux operations fail.
pub fn startup() -> Result<()> {
    match env::var("TMUX") {
        Ok(val) if !val.is_empty() => {
            let default_session_exists = utils::tmux_session_exisits(DEFAULT_SESSION)
                .with_context(|| {
                    format!("Error checking if default session '{DEFAULT_SESSION}' exists")
                })?;
            if default_session_exists {
                utils::tmux_display_message(&format!(
                    "The default session '{DEFAULT_SESSION}' is already running"
                ))
                .context("Error sending 'Default session already running' notification")?;
            } else {
                create_default_session().with_context(|| {
                    format!("Error creating default session '{DEFAULT_SESSION}'")
                })?;
                utils::tmux_switch_client(DEFAULT_SESSION, Some(1)).with_context(|| {
                    format!(
                        "Error switching to first window of default session '{DEFAULT_SESSION}'"
                    )
                })?;
            }
        }
        _ => {
            create_default_session()
                .with_context(|| format!("Error creating default session '{DEFAULT_SESSION}'"))?;
            utils::tmux_command_without_output(&[
                "attach-session",
                "-t",
                &format!("{DEFAULT_SESSION}:1"),
            ])
            .with_context(|| format!("Error attaching to default session '{DEFAULT_SESSION}'"))?;
        }
    }

    Ok(())
}

fn create_default_session() -> Result<()> {
    let home = env::var("HOME").context("Error getting $HOME")?;
    utils::tmux_command_without_output(&["new-session", "-ds", DEFAULT_SESSION, "-c", &home])
        .with_context(|| format!("Error creating default tmux session '{DEFAULT_SESSION}'"))?;
    utils::tmux_command_without_output(&[
        "new-window",
        "-t",
        &format!("{DEFAULT_SESSION}:2"),
        "-c",
        &home,
    ])
    .with_context(|| {
        format!("Error creating second window for default session '{DEFAULT_SESSION}'")
    })?;

    Ok(())
}
