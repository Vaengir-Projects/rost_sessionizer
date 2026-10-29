//!
//! # Open handler
//!
//! This module handles the logic to use fzf to create a new or open an existing session.

use crate::{
    commands::cli::SearchMode,
    config,
    db::{self},
    utils::{sanitize_selections, tmux},
};
use anyhow::{Context, Result};
use clap::parser::ValuesRef;
use std::{
    collections::{HashMap, hash_map::Entry},
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

/// Opens the selected session.
///
/// # Errors
///
/// Returns `io::Error` if any of the tmux commands fail, or
/// `anyhow::Error` if parsing the existing sessions fails.
pub fn open(search_modes: ValuesRef<'_, SearchMode>) -> Result<()> {
    let session_names =
        tmux::existing_session_names().context("Error getting existing session names")?;

    let mut possible_selections: Dirs = HashMap::new();
    possible_selections
        .entry(config::default_session())
        .or_insert(Dir::from((config::default_session(), None, None)));

    for session_name in session_names {
        if session_name != config::default_session() {
            possible_selections
                .entry(session_name.clone())
                .or_insert(Dir::from((session_name, None, None)));
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

    let sanitized_selections = sanitize_selections(possible_selections);

    let selected = select_via_fzf(&sanitized_selections.sort())
        .context("Error selecting new or existing session")?;

    let existing_session = tmux::tmux_session_exisits(&selected.name)
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
            dirs.entry(name.clone()).or_insert(Dir::from((
                name,
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
            dirs.entry(name.clone()).or_insert(Dir::from((
                name,
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
                    dirs.entry(name.clone()).or_insert(Dir::from((
                        name,
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
                        dirs.entry(name.clone()).or_insert(Dir::from((
                            name,
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
fn select_via_fzf(possible_selections: &Vec<(String, Dir)>) -> Result<Dir> {
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
    for (possible_name, _) in possible_selections {
        if existing_session_names.contains(possible_name) {
            // Display already open sessions in bold.
            writeln!(stdin, "\x1b[1m{possible_name}\x1b[0m")?;
        } else {
            writeln!(stdin, "{possible_name}")?;
        }
    }

    let selected = child
        .wait_with_output()
        .context("Error reading fzf stdout")?
        .stdout;
    let selected = String::from_utf8_lossy(&selected);
    let selected = selected.trim();
    let (_, selected_dir) = possible_selections
        .iter()
        .find(|(name, _path)| *name == selected)
        .context("Selected value isn't part of provided options")?;

    Ok(selected_dir.clone())
}

trait HashMapExtend {
    fn try_extend(&mut self, iter: Self);

    fn sort(&self) -> Vec<(String, Dir)>;
}

pub(crate) type Dirs = HashMap<String, Dir>;

impl HashMapExtend for Dirs {
    fn try_extend(&mut self, iter: Self) {
        for (k, v) in iter {
            match self.entry(k) {
                Entry::Vacant(e) => {
                    e.insert(v);
                }
                Entry::Occupied(mut e) => {
                    if e.get().rank.is_none() && v.rank.is_some() {
                        e.insert(v);
                    }
                }
            }
        }
    }

    fn sort(&self) -> Vec<(String, Dir)> {
        let mut sorted_vec: Vec<(String, Dir)> =
            self.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        sorted_vec.sort_by(|(ka, va), (kb, vb)| {
            if ka == &config::default_session() {
                return std::cmp::Ordering::Less;
            }
            if kb == &config::default_session() {
                return std::cmp::Ordering::Greater;
            }

            match (va.rank, vb.rank) {
                (None, Some(_)) => std::cmp::Ordering::Less,
                (Some(_), None) => std::cmp::Ordering::Greater,
                (Some(ra), Some(rb)) => rb.total_cmp(&ra),
                (None, None) => match (va.path.clone(), vb.path.clone()) {
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    _ => ka.cmp(kb),
                },
            }
        });

        sorted_vec
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Dir {
    pub(crate) name: String,
    pub(crate) path: Option<PathBuf>,
    pub(crate) rank: Option<f64>,
}

impl From<(String, Option<PathBuf>, Option<f64>)> for Dir {
    fn from((name, path, rank): (String, Option<PathBuf>, Option<f64>)) -> Self {
        Dir { name, path, rank }
    }
}

impl PartialEq for Dir {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
