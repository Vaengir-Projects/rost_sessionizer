use crate::utils::types::session_name::SessionName;
use anyhow::{Context, Result};
use std::{env, path::PathBuf};

pub(crate) fn default_session() -> SessionName {
    let raw = env::var("DEFAULT_SESSION").unwrap_or_else(|_| "Default".to_string());

    SessionName::new(&raw)
}

pub(crate) fn paths() -> Result<Vec<PathBuf>> {
    let paths = env::var_os("SESSIONIZER_PATHS").context("SESSIONIZER_PATHS is not set")?;

    Ok(env::split_paths(&paths).collect())
}
