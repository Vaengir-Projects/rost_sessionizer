//!
//! # Frecency handler
//!
//! This module handles the logic to manage the frecency database.

use crate::{commands::open::Dir, db, utils};
use anyhow::{Context, Result};
use clap::ArgMatches;
use std::path::PathBuf;

/// Kills the session the user is currently attached to.
///
/// # Errors
///
/// Returns `anyhow::Error` if anything goes wrong when managing the frecency database.
pub fn frecency(sub_matches: &ArgMatches) -> Result<()> {
    match sub_matches.subcommand() {
        Some(("new", sm)) => new(sm),
        Some(("add", sm)) => add(sm),
        Some(("remove", sm)) => remove(sm),
        Some(("modify", sm)) => modify(sm),
        Some(("list", _)) => list(),
        e => unreachable!("Should be unreachable!: {:?}", e),
    }
}

fn new(sub_matches: &ArgMatches) -> Result<()> {
    let name = sub_matches
        .get_one::<String>("name")
        .context("Error getting the name value from the input")?;
    let path = sub_matches
        .get_one::<PathBuf>("path")
        .context("Error getting the path value from the input")?;
    let absolute_path = path
        .canonicalize()
        .context("Error canonicalizing the given path")?;
    let dir = Dir::from((name.clone(), Some(absolute_path), None));

    let conn = db::open().context("Error getting database conntection")?;

    db::new(&conn, &dir)
        .with_context(|| format!("Error adding dir: '{dir:#?}' to frecency database"))
}

fn add(sub_matches: &ArgMatches) -> Result<()> {
    let path = std::env::current_dir()
        .context("Error getting current directory")?
        .canonicalize()
        .context("Error canonicalizing the given path")?;
    let name = if let Some(name) = sub_matches.get_one::<String>("name") {
        name.clone()
    } else {
        let dir_name = path
            .file_name()
            .context("The current directory has no name (is it '/'?), please pass one")?
            .to_string_lossy();
        utils::validate_name(&utils::sanitize_session_name(&dir_name))
            .context("Error deriving a name from the current directory")?
    };

    let dir = Dir::from((name, Some(path), None));
    let conn = db::open().context("Error getting database conntection")?;

    db::new(&conn, &dir)
        .with_context(|| format!("Error adding dir: '{dir:#?}' to frecency database"))
}

fn modify(sub_matches: &ArgMatches) -> Result<()> {
    let old_name = sub_matches
        .get_one::<String>("old_name")
        .context("Error getting the old_name value from the input")?;
    let new_name = sub_matches
        .get_one::<String>("new_name")
        .context("Error getting the new_name value from the input")?;

    let conn = db::open().context("Error getting database conntection")?;

    db::modify(&conn, old_name, new_name)
        .with_context(|| format!("Error updating entry: '{old_name}' with '{new_name}'"))
}

fn remove(sub_matches: &ArgMatches) -> Result<()> {
    let name = sub_matches
        .get_one::<String>("name")
        .context("Error getting the name value from the input")?;

    let conn = db::open().context("Error getting database conntection")?;

    db::remove(&conn, name)
        .with_context(|| format!("Error removing entry: '{name:#?}' from frecency database"))
}

fn list() -> Result<()> {
    let conn = db::open().context("Error getting database conntection")?;

    db::select_all(&conn).context("Error printing frecency database entries")?;

    Ok(())
}
