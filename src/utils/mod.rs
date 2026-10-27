//!
//! # Utils section
//!
//! This module provides functions that are used internally.

use crate::commands::open::Dirs;
use anyhow::{Context, Result};
use std::process::Command;

/// Module that provides internally used tmux wrappers.
pub(crate) mod tmux;

pub(crate) fn get_client_terminal_size() -> Result<(u16, u16)> {
    let coords = Command::new("tmux")
        .args(["display-message", "-p", "#{client_width}x#{client_height}"])
        .output()
        .context("Error getting client terminal size")?
        .stdout;
    let coords = String::from_utf8_lossy(&coords).to_string();

    let (w, h) = coords
        .trim()
        .split_once('x')
        .context("Unexpected format from tmux display-message")?;

    Ok((
        w.parse().context("Error parsing cols")?,
        h.parse().context("Error parsing rows")?,
    ))
}

pub(crate) fn sanitize_selections(selections: Dirs) -> Dirs {
    selections
        .into_iter()
        .map(|(k, v)| (sanitize_session_name(&k), v))
        .collect()
}

pub(crate) fn sanitize_session_name(name: &str) -> String {
    name.chars()
        .map(|c| if c == '.' || c == ':' { '_' } else { c })
        .collect()
}
