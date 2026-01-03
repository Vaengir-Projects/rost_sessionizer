use std::{env, path::PathBuf};

pub(crate) fn default_session() -> String {
    match env::var_os("DEFAULT_SESSION") {
        Some(default_session) => default_session.to_str().unwrap_or("Default").to_string(),
        None => "Default".to_string(),
    }
}

pub(crate) fn paths() -> Vec<PathBuf> {
    match env::var_os("SESSIONIZER_PATHS") {
        Some(paths) => env::split_paths(&paths).collect(),
        None => panic!("No PATHS configured"),
    }
}
