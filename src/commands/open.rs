//!
//! # Open handler
//!
//! This module handles the logic to use fzf to create a new or open an existing session.

use crate::{
    commands::cli::SearchMode,
    config,
    db::{self},
    utils::{
        tmux,
        types::{
            dir::{Dir, Dirs, HashMapExtend as _},
            session_name::SessionName,
        },
    },
};
use anyhow::{Context, Result};
use clap::parser::ValuesRef;
use std::{
    collections::HashMap,
    io::Write,
    process::{Command, Stdio},
};

/// Opens the selected session.
///
/// # Errors
///
/// Returns `io::Error` if any of the tmux commands fail, or
/// `anyhow::Error` if parsing the existing sessions fails.
pub fn open(search_modes: ValuesRef<'_, SearchMode>) -> Result<()> {
    let default_session = config::default_session();
    let session_names =
        tmux::existing_session_names().context("Error getting existing session names")?;

    let mut possible_selections: Dirs = HashMap::new();
    possible_selections.try_insert_dir(Dir::from((default_session.clone(), None, None)));

    for session_name in session_names {
        if session_name != default_session {
            possible_selections.try_insert_dir(Dir::from((session_name, None, None)));
        }
    }

    // Open connection to frecency database
    let mut conn = db::open().context("Error getting database conntection")?;

    for search_mode in search_modes {
        match search_mode {
            SearchMode::All => {
                // Add all Directories
                possible_selections
                    .try_extend(get_directories().context("Error finding all directories")?);

                // Add all Repos
                possible_selections.try_extend(get_repos().context("Error finding all repos")?);

                // Add all Worktrees
                possible_selections.try_extend(get_worktrees().context("Error finding all repos")?);

                // Add all from Frecency db
                possible_selections
                    .try_extend(db::scores(&conn).context("Error getting paths by frecency")?);
            }
            SearchMode::Dirs => {
                possible_selections
                    .try_extend(get_directories().context("Error finding all directories")?);
            }
            SearchMode::Repos => {
                possible_selections.try_extend(get_repos().context("Error finding all repos")?);
            }
            SearchMode::Worktrees => {
                possible_selections.try_extend(get_worktrees().context("Error finding all repos")?);
            }
            SearchMode::Frecency => {
                possible_selections
                    .try_extend(db::scores(&conn).context("Error getting paths by frecency")?);
            }
        }
    }

    let selected = select_via_fzf(&possible_selections.sort())
        .context("Error selecting new or existing session")?;

    let existing_session = tmux::tmux_session_exists(&selected.name)
        .with_context(|| format!("Error checking if session '{}' exists", selected.name))?;

    if existing_session {
        tmux::tmux_switch_client(&selected.name, Some(1))
            .context("Error switching to existing session")?;
    } else {
        tmux::create_tmux_session(&selected).context("Error creating new tmux session")?;
    }

    db::bump(&conn, &selected).context("Error updating frecency for selected Dir")?;
    db::age(&mut conn).context("Error aging frecency database")?;

    Ok(())
}

fn get_directories() -> Result<Dirs> {
    let mut dirs: Dirs = Dirs::new();
    // Iterate over configured paths and parse them.
    config::paths().iter().try_for_each(|path| {
        let absolute_path = path.canonicalize().context("Error canonicalizing path")?;

        if !absolute_path.join(".git").exists() {
            let name = absolute_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            dirs.try_insert_dir(Dir::from((
                SessionName::new(&name),
                Some(absolute_path.clone()),
                None,
            )));
        }

        Ok::<_, anyhow::Error>(())
    })?;

    Ok(dirs)
}

fn get_repos() -> Result<Dirs> {
    let mut dirs: Dirs = Dirs::new();
    // Iterate over configured paths, check if they are git repositories and parse them.
    config::paths().iter().try_for_each(|path| {
        let absolute_path = path.canonicalize().context("Error canonicalizing path")?;

        if absolute_path.join(".git").exists() {
            let name = absolute_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            dirs.try_insert_dir(Dir::from((
                SessionName::new(&name),
                Some(absolute_path.clone()),
                None,
            )));
        }

        Ok::<_, anyhow::Error>(())
    })?;

    Ok(dirs)
}

fn get_worktrees() -> Result<Dirs> {
    let mut dirs: Dirs = Dirs::new();
    // Iterate over configured paths, check if they are bare git repositories, find the worktrees and parse them.
    config::paths().iter().try_for_each(|path| {
        let absolute_path = path.canonicalize().context("Error canonicalizing path")?;

        let child_dirs = absolute_path.read_dir().with_context(|| {
            format!(
                "Couldn't get the child directories of {}",
                absolute_path.display()
            )
        })?;
        for child_dir in child_dirs {
            let dir = child_dir.context("Child directory has an error")?;
            if dir.file_type()?.is_dir() {
                if let ".git" = dir
                    .file_name()
                    .to_str()
                    .context("Error converting filename to str")?
                {
                    let mut path = dir.path();
                    path.pop();

                    let name = path.file_name().unwrap().to_string_lossy().to_string();
                    dirs.try_insert_dir(Dir::from((
                        SessionName::new(&name),
                        Some(
                            path.canonicalize()
                                .context("Error canonicalizing path")?
                                .clone(),
                        ),
                        None,
                    )));
                } else {
                    let path = dir.path().join(".git");
                    if path.try_exists()? {
                        let absolute_path = dir
                            .path()
                            .canonicalize()
                            .context("Error canonicalizing path")?;
                        let mut p = absolute_path.components().rev();
                        let worktree = p
                            .next()
                            .context("Error getting worktree name")?
                            .as_os_str()
                            .to_string_lossy();
                        let base = p
                            .next()
                            .context("Error getting base name")?
                            .as_os_str()
                            .to_string_lossy();
                        let name = format!("{base}/{worktree}");
                        dirs.try_insert_dir(Dir::from((
                            SessionName::new(&name),
                            Some(absolute_path),
                            None,
                        )));
                    }
                }
            }
        }

        Ok::<_, anyhow::Error>(())
    })?;

    Ok(dirs)
}

/// Allows user to select the session they want to open.
///
/// # Errors
///
/// Returns `io::Error` if the fzf or any of the tmux commands fail, or
/// `anyhow::Error` if anything with stdin or stdout fails.
fn select_via_fzf(possible_selections: &[Dir]) -> Result<Dir> {
    let mut child = Command::new("fzf")
        .args([
            "--margin=5%",
            "--padding=2%",
            "--border",
            "--ansi",
            "--tiebreak=index",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to spawn fzf")?;

    let stdin = child.stdin.as_mut().context("Error opening fzf stdin")?;

    let existing_session_names =
        tmux::existing_session_names().context("Error getting existing session names")?;
    for dir in possible_selections {
        let name = &dir.name;
        if existing_session_names.contains(name) {
            // Display already open sessions in bold.
            writeln!(stdin, "\x1b[1m{name}\x1b[0m")?;
        } else {
            writeln!(stdin, "{name}")?;
        }
    }

    let selected = child
        .wait_with_output()
        .context("Error reading fzf stdout")?
        .stdout;
    let selected = String::from_utf8_lossy(&selected);
    let selected = selected.trim();
    possible_selections
        .iter()
        .find(|dir| dir.name.as_str() == selected)
        .cloned()
        .context("Selected value isn't part of provided options")
}
