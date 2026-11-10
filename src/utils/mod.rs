//!
//! # Utils section
//!
//! This module provides functions that are used internally.

use anyhow::{Context, Result, bail};
use std::process::Command;

use self::types::session_name::SessionName;

/// Module that provides internally used tmux wrappers.
pub(crate) mod tmux;

/// Module that provides internally used types.
pub(crate) mod types;

pub(crate) fn get_client_terminal_size() -> Result<(u16, u16)> {
    let output = Command::new("tmux")
        .args(["display-message", "-p", "#{client_width}x#{client_height}"])
        .output()
        .context("Error getting client terminal size")?;
    if !output.status.success() {
        bail!(
            "tmux display-message failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let coords = String::from_utf8_lossy(&output.stdout);
    let (w, h) = coords
        .trim()
        .split_once('x')
        .context("Unexpected format from tmux display-message")?;

    Ok((
        w.parse().context("Error parsing cols")?,
        h.parse().context("Error parsing rows")?,
    ))
}

pub(crate) fn validate_name(name: &str) -> Result<SessionName> {
    if name.trim().is_empty() {
        bail!("Error: name must contain at least one non-whitespace character");
    }
    // This is a limitation by tmux. See #20260911-210534
    if name.contains(['.', ':']) {
        bail!("Error: name must not contain '.' or ':', use '_' or '-' instead");
    }

    Ok(SessionName::new(name))
}
