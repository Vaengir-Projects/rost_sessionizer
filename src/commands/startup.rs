use crate::{config, utils};
use anyhow::{Context, Result};
use std::env;

/// # Errors
///
/// Will return `Err` if any of the tmux operations fail.
pub fn startup() -> Result<()> {
    match env::var("TMUX") {
        Ok(val) if !val.is_empty() => {
            let default_session_exists = utils::tmux_session_exisits(&config::default_session())
                .with_context(|| {
                    format!(
                        "Error checking if default session '{}' exists",
                        &config::default_session()
                    )
                })?;
            if default_session_exists {
                utils::tmux_display_message(&format!(
                    "The default session '{}' is already running",
                    &config::default_session()
                ))
                .context("Error sending 'Default session already running' notification")?;
            } else {
                create_default_session().with_context(|| {
                    format!(
                        "Error creating default session '{}'",
                        &config::default_session()
                    )
                })?;
                utils::tmux_switch_client(&config::default_session(), Some(1)).with_context(
                    || {
                        format!(
                            "Error switching to first window of default session '{}'",
                            &config::default_session()
                        )
                    },
                )?;
            }
        }
        _ => {
            create_default_session().with_context(|| {
                format!(
                    "Error creating default session '{}'",
                    &config::default_session()
                )
            })?;
            utils::tmux_command_without_output(&[
                "attach-session",
                "-t",
                &format!("{}:1", &config::default_session()),
            ])
            .with_context(|| {
                format!(
                    "Error attaching to default session '{}'",
                    &config::default_session()
                )
            })?;
        }
    }

    Ok(())
}

fn create_default_session() -> Result<()> {
    let home = env::var("HOME").context("Error getting $HOME")?;
    utils::tmux_command_without_output(&[
        "new-session",
        "-ds",
        &config::default_session(),
        "-c",
        &home,
    ])
    .with_context(|| {
        format!(
            "Error creating default tmux session '{}'",
            &config::default_session()
        )
    })?;
    utils::tmux_command_without_output(&[
        "new-window",
        "-t",
        &format!("{}:2", &config::default_session()),
        "-c",
        &home,
    ])
    .with_context(|| {
        format!(
            "Error creating second window for default session '{}'",
            config::default_session()
        )
    })?;

    Ok(())
}
