//!
//! # Open handler
//!
//! This module handles the logic to use fzf to create a new or open an existing session.

use crate::{DEFAULT_SESSION, PATHS, commands::cli::SearchMode, utils};
use anyhow::{Context, Result, anyhow};
use std::{
    collections::HashMap,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

/// # Errors
///
/// Will return `Err` if the existing sessions can't be found, an error with selecting a value from
/// the possible selections occurs or any of the tmux operations fail.
pub fn open(search_mode: &SearchMode) -> Result<()> {
    let session_names =
        utils::existing_session_names().context("Error getting existing session names")?;

    let mut possible_selections: Dirs = HashMap::new();
    possible_selections
        .entry(DEFAULT_SESSION.to_string())
        .or_insert(None);

    for session_name in session_names {
        if session_name != DEFAULT_SESSION {
            possible_selections.entry(session_name).or_insert(None);
        }
    }

    match search_mode {
        SearchMode::All => {
            // Add all Directories
            possible_selections
                .try_extend(get_directories().context("Error finding all directories")?);

            // Add all Repos
            possible_selections.try_extend(get_repos().context("Error finding all repos")?);

            // Add all Worktrees
            possible_selections.try_extend(get_worktrees().context("Error finding all repos")?);
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
    }

    let selected = select_via_fzf(&possible_selections.sort())
        .context("Error selecting new or existing session")?;

    let existing_session = utils::tmux_session_exisits(&selected.name)
        .with_context(|| format!("Error checking if session '{}' exists", selected.name))?;

    if existing_session {
        utils::tmux_switch_client(&selected.name, Some(1))
            .context("Error switching to existing session")?;
    } else {
        create_tmux_session(&selected).context("Error creating new tmux session")?;
    }

    Ok(())
}

fn get_directories() -> Result<Dirs> {
    let mut dirs: Dirs = Dirs::new();
    // Iterate over configured paths and parse them.
    PATHS.iter().try_for_each(|path| {
        let path = PathBuf::from(path);
        dirs.entry(
            path.file_name()
                .ok_or(anyhow!("No file_name provided"))?
                .to_string_lossy()
                .to_string(),
        )
        .or_insert(Some(path.clone()));

        Ok::<_, anyhow::Error>(())
    })?;

    Ok(dirs)
}

fn get_repos() -> Result<Dirs> {
    let mut dirs: Dirs = Dirs::new();
    // Iterate over configured paths, check if they are git repositories and parse them.
    PATHS.iter().try_for_each(|path| {
        let child_dirs = PathBuf::from(path)
            .canonicalize()?
            .read_dir()
            .with_context(|| format!("Couldn't get the child directories of {}", &path))?;
        for child_dir in child_dirs {
            let dir = child_dir.context("Child directory has an error")?;
            if let ".git" = dir
                .file_name()
                .to_str()
                .context("Error converting filename to str")?
            {
                let mut path = dir.path();
                path.pop();

                dirs.entry(path.file_name().unwrap().to_string_lossy().to_string())
                    .or_insert(Some(path.clone()));
            }
        }

        Ok::<_, anyhow::Error>(())
    })?;

    Ok(dirs)
}

fn get_worktrees() -> Result<Dirs> {
    let mut dirs: Dirs = Dirs::new();
    // Iterate over configured paths, check if they are bare git repositories, find the worktrees and parse them.
    PATHS.iter().try_for_each(|path| {
        let child_dirs = PathBuf::from(path)
            .canonicalize()?
            .read_dir()
            .with_context(|| format!("Couldn't get the child directories of {}", &path))?;
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

                    dirs.entry(path.file_name().unwrap().to_string_lossy().to_string())
                        .or_insert(Some(path.clone()));
                } else {
                    let path = PathBuf::from(format!(
                        "{}/.git",
                        dir.path()
                            .to_str()
                            .context("Error appending `.git` to given path")?
                    ));
                    if path.try_exists()? {
                        let p = dir.path().clone();
                        let mut p = p.iter();
                        let worktree = p
                            .next_back()
                            .context("Error getting worktree name")?
                            .to_string_lossy()
                            .to_string();
                        let base = p
                            .next_back()
                            .context("Error getting base name")?
                            .to_string_lossy()
                            .to_string();
                        dirs.entry(format!("{base}/{worktree}"))
                            .or_insert(Some(dir.path()));
                    }
                }
            }
        }

        Ok::<_, anyhow::Error>(())
    })?;

    Ok(dirs)
}

fn create_tmux_session(selected_session: &Dir) -> Result<()> {
    // Create Session
    utils::tmux_command_without_output(&[
        "new-session",
        "-ds",
        &selected_session.name,
        "-c",
        &selected_session.path.clone().unwrap().to_string_lossy(),
    ])
    .context("Error creating tmux session")?;
    // Setup Window Layout
    utils::tmux_command_without_output(&[
        "rename-window",
        "-t",
        &format!("{}:1", &selected_session.name),
        "Neovim",
    ])
    .context("Error renaming first window")?;
    utils::tmux_command_without_output(&[
        "new-window",
        "-t",
        &format!("{}:2", &selected_session.name),
        "-c",
        &selected_session.path.clone().unwrap().to_string_lossy(),
    ])
    .context("Error creating second window")?;
    utils::tmux_command_without_output(&[
        "rename-window",
        "-t",
        &format!("{}:2", &selected_session.name),
        "Bash",
    ])
    .context("Error renaming second window")?;
    utils::tmux_switch_client(&selected_session.name, Some(1))
        .context("Error switching back to first window")?;
    utils::tmux_command_without_output(&[
        "send-keys",
        "-t",
        &format!("{}:1", &selected_session.name),
        "v",
        "Enter",
    ])
    .context("Error starting Neovim")?;

    Ok(())
}

fn select_via_fzf(possible_selections: &Vec<(String, Option<PathBuf>)>) -> Result<Dir> {
    let mut child = Command::new("fzf")
        .args(["--margin=5%", "--padding=2%", "--border", "--ansi"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to spawn fzf")?;

    let stdin = child.stdin.as_mut().context("Error opening fzf stdin")?;
    for (possible_name, possible_path) in possible_selections {
        if possible_path.is_none() {
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
    let (selected_name, selected_path) = possible_selections
        .iter()
        .find(|(name, _path)| *name == selected)
        .context("Selected value isn't part of provided options")?;

    Ok(Dir::from((selected_name.clone(), selected_path.clone())))
}

trait HashMapExtend {
    fn try_extend(&mut self, iter: Self);

    fn sort(&self) -> Vec<(String, Option<PathBuf>)>;
}

type Dirs = HashMap<String, Option<PathBuf>>;

impl HashMapExtend for Dirs {
    fn try_extend(&mut self, iter: Self) {
        for (k, v) in iter {
            self.entry(k).or_insert(v);
        }
    }

    fn sort(&self) -> Vec<(String, Option<PathBuf>)> {
        let mut sorted_vec: Vec<(String, Option<PathBuf>)> =
            self.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        sorted_vec.sort_by(|(ka, va), (kb, vb)| {
            if ka == DEFAULT_SESSION {
                return std::cmp::Ordering::Less;
            }
            if kb == DEFAULT_SESSION {
                return std::cmp::Ordering::Greater;
            }

            match (va, vb) {
                (None, Some(_)) => std::cmp::Ordering::Less,
                (Some(_), None) => std::cmp::Ordering::Greater,
                _ => ka.cmp(kb),
            }
        });

        sorted_vec
    }
}

#[derive(Debug, Clone)]
struct Dir {
    name: String,
    path: Option<PathBuf>,
}

impl From<(String, Option<PathBuf>)> for Dir {
    fn from((name, path): (String, Option<PathBuf>)) -> Dir {
        Dir { name, path }
    }
}

impl PartialEq for Dir {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
