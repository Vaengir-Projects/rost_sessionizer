//!
//! # Utils section
//!
//! This module provides functions that are used internally.

use anyhow::{Result, bail};

use self::types::session_name::SessionName;

/// Module that provides internally used tmux wrappers.
pub(crate) mod tmux;

/// Module that provides internally used types.
pub(crate) mod types;
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
