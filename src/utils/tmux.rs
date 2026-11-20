//!
//! # Tmux utils section
//!
//! This module provides tmux wrapper functions that are used internally.

use crate::utils::types::{dir::Dir, session_name::SessionName};
use anyhow::{Context, Result, bail};
use std::{
    env,
    path::PathBuf,
    process::{Command, Stdio},
};

pub(crate) fn tmux_command_without_output(args: &[&str]) -> Result<()> {
    let mut command = Command::new("tmux");
    command.args(args);
    let status = command
        .status()
        .with_context(|| format!("Error running {command:?}"))?;

    if !status.success() {
        bail!("{command:?} failed with {status}");
    }

    Ok(())
}

pub(crate) fn tmux_command_with_output(args: &[&str]) -> Result<String> {
    let mut command = Command::new("tmux");
    command.args(args);
    let output = command
        .output()
        .with_context(|| format!("Error running {command:?}"))?;

    if !output.status.success() {
        bail!(
            "{command:?} failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub(crate) fn tmux_switch_client(target_session: &SessionName) -> Result<()> {
    tmux_command_without_output(&[
        "switch-client",
        "-t",
        &format!("{}:^", target_session.target()),
    ])
    .with_context(|| format!("Error switching tmux client to '{target_session}'"))?;

    Ok(())
}

pub(crate) fn tmux_kill_session(target_session: &SessionName) -> Result<()> {
    tmux_command_without_output(&["kill-session", "-t", &target_session.target()])
        .with_context(|| format!("Error killing session '{target_session}'"))?;

    Ok(())
}

pub(crate) fn tmux_kill_other_sessions(keep: &SessionName) -> Result<()> {
    tmux_command_without_output(&["kill-session", "-a", "-t", &keep.target()])
        .with_context(|| format!("Error killing all sessions except '{keep}'"))
}

pub(crate) fn tmux_display_message(message: &str) -> Result<()> {
    tmux_command_without_output(&["display-message", message]).context("Error sending notification")
}

pub(crate) fn tmux_session_exists(target_session: &SessionName) -> Result<bool> {
    Command::new("tmux")
        .args(["has-session", "-t", &target_session.target()])
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .context("Error checking if session exists")
}

pub(crate) fn current_session() -> Result<SessionName> {
    // Outside tmux, `display-message` falls back to the most recently used session instead of
    // failing, which would make `kill` target a session it wasn't called from.
    if env::var_os("TMUX").is_none_or(|v| v.is_empty()) {
        bail!("Not running inside tmux, so there is no current session");
    }

    let output = tmux_command_with_output(&["display-message", "-p", "#{session_name}"])
        .context("Error asking tmux for the current session")?;

    parse_session_name(&output)
}

fn parse_session_name(output: &str) -> Result<SessionName> {
    let name = output.trim();
    if name.is_empty() {
        bail!("tmux didn't report a current session");
    }

    Ok(SessionName::new(name))
}

pub(crate) fn existing_sessions() -> Result<Vec<Dir>> {
    let output =
        tmux_command_with_output(&["list-sessions", "-F", "#{session_name}:#{session_path}"])
            .context("Error listing existing tmux sessions")?;

    parse_sessions(&output)
}

fn parse_sessions(output: &str) -> Result<Vec<Dir>> {
    output
        .lines()
        .map(|line| {
            let (name, path) = line
                .split_once(':')
                .with_context(|| format!("Unexpected line from tmux list-sessions: '{line}'"))?;
            Ok(Dir::session(SessionName::new(name), PathBuf::from(path)))
        })
        .collect()
}

pub(crate) fn create_tmux_session(selected_session: &Dir) -> Result<()> {
    let path_str = selected_session.path.to_string_lossy();
    let (cols, rows) = client_terminal_size().context("Error getting client terminal size")?;

    // Create Session
    tmux_command_without_output(&[
        "new-session",
        "-ds",
        selected_session.name.as_str(),
        "-n",
        "Neovim",
        "-c",
        &path_str,
        "-x",
        &cols.to_string(),
        "-y",
        &rows.to_string(),
        "nvim; exec $SHELL",
    ])
    .context("Error creating tmux session")?;
    tmux_command_without_output(&[
        "new-window",
        "-t",
        &format!("{}:", selected_session.name.target()),
        "-n",
        "Bash",
        "-c",
        &path_str,
    ])
    .context("Error creating second window")?;
    tmux_switch_client(&selected_session.name).context("Error switching back to first window")?;

    Ok(())
}

fn client_terminal_size() -> Result<(u16, u16)> {
    let output =
        tmux_command_with_output(&["display-message", "-p", "#{client_width}x#{client_height}"])
            .context("Error asking tmux for the client size")?;

    parse_terminal_size(&output)
}

fn parse_terminal_size(output: &str) -> Result<(u16, u16)> {
    let (w, h) = output
        .trim()
        .split_once('x')
        .context("Unexpected format from tmux display-message")?;

    Ok((
        w.parse().context("Error parsing cols")?,
        h.parse().context("Error parsing rows")?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::fmt::Write;

    mod parse_session_name {
        use super::*;

        #[test]
        fn rejects_empty_output() {
            for output in ["", "\n", "  \n"] {
                assert!(parse_session_name(output).is_err(), "{output:?}");
            }
        }

        proptest! {
            #[test]
            fn round_trip(name in "[^:.\\s]([^:.\n\r]{0,18}[^:.\\s])?") {
                let output = format!("{name}\n");
                let parsed = parse_session_name(&output).unwrap();
                prop_assert_eq!(parsed.as_str(), name.as_str());
            }
        }
    }

    mod parse_sessions {
        use super::*;

        #[test]
        fn empty_output_no_sessions() {
            assert_eq!(parse_sessions("").unwrap(), []);
        }

        #[test]
        fn first_colon_separates_name_and_path() {
            let sessions = parse_sessions("repo:/home/user/a:b\n").unwrap();
            assert_eq!(sessions[0].name.as_str(), "repo");
            assert_eq!(sessions[0].path, PathBuf::from("/home/user/a:b"));
        }

        #[test]
        fn rejects_lines_without_colon() {
            let error = parse_sessions("repo\n").unwrap_err();
            assert!(error.to_string().contains("'repo'"));
        }

        proptest! {
            #[test]
            fn round_trip(
                sessions in prop::collection::vec(("[^:.\n\r]{1,20}", "/[^\n\r]{0,30}"), 0..10),
            ) {
                let output: String = sessions
                    .iter()
                    .fold(String::new(), |mut output, (name, path)| {
                        let _ = writeln!(output, "{name}:{path}");
                        output
                    });
                let parsed = parse_sessions(&output).unwrap();
                prop_assert_eq!(parsed.len(), sessions.len());
                for (dir, (name, path)) in parsed.iter().zip(&sessions) {
                    prop_assert_eq!(dir.name.as_str(), name.as_str());
                    prop_assert_eq!(&dir.path, &PathBuf::from(path));
                    prop_assert!(dir.has_session);
                }
            }
        }
    }

    mod parse_terminal_size {
        use super::*;

        #[test]
        fn rejects_malformed_output() {
            for output in [
                "", "x", "80", "80x", "x24", "-1x24", "70000x24", "80x24x1", "abcxdef",
            ] {
                assert!(parse_terminal_size(output).is_err(), "{output:?}");
            }
        }

        proptest! {
            #[test]
            fn round_trip(width in any::<u16>(), height in any::<u16>()) {
                let output = format!("{width}x{height}\n");
                prop_assert_eq!(parse_terminal_size(&output).unwrap(), (width, height));
            }

            #[test]
            fn never_panics(output in any::<String>()) {
                let _ = parse_terminal_size(&output);
            }
        }
    }
}
