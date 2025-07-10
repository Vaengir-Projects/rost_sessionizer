use crate::DEFAULT_SESSION;
use anyhow::{Context, Result, anyhow};
use std::process::Command;

pub(crate) fn kill_current_session() -> Result<()> {
    let existing_sessions = Command::new("tmux")
        .arg("ls")
        .output()
        .context("Error listing existing tmux sessions")?
        .stdout;
    let existing_sessions = String::from_utf8_lossy(&existing_sessions);
    let current_session = existing_sessions
        .lines()
        .map(|s| s.to_string())
        .find(|s| s.contains("attached"))
        .context("No current session available")?;

    let current_session = match current_session.find(':') {
        Some(i) => &current_session[..i],
        None => return Err(anyhow!("Error parsing current session name")),
    };

    Command::new("tmux")
        .args(["switch-client", "-t", &format!("{DEFAULT_SESSION}:1")])
        .status()
        .context("Error switching to default session")?;

    if current_session != DEFAULT_SESSION {
        Command::new("tmux")
            .args(["kill-session", "-t", current_session])
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

