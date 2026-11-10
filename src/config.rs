use crate::utils::types::session_name::SessionName;
use std::{env, path::PathBuf};

pub(crate) fn default_session() -> SessionName {
    let raw = env::var("DEFAULT_SESSION").unwrap_or_else(|_| "Default".to_string());

    SessionName::new(&raw)
}

pub(crate) fn paths() -> Vec<PathBuf> {
    match env::var_os("SESSIONIZER_PATHS") {
        Some(paths) => env::split_paths(&paths).collect(),
        None => panic!("No PATHS configured"),
    }
}
