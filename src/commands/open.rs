//!
//! # Open handler
//!
//! This module handles the logic to use fzf to create a new or open an existing session.

use crate::{DEFAULT_SESSION, PATHS, commands::cli::GitMode, utils};
use anyhow::{Context, Result};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

pub fn open(git: bool, _git_mode: Option<&GitMode>) -> Result<()> {
    let session_names =
        utils::existing_session_names().context("Error getting existing session names")?;

    let mut sorted_existing_sessions: Dirs = Dirs {
        dirs: vec![Dir {
            name: String::from(DEFAULT_SESSION),
            path: None,
        }],
    };
    for session_name in session_names {
        if session_name != DEFAULT_SESSION {
            sorted_existing_sessions.dirs.push(Dir {
                name: session_name.to_string(),
                path: None,
            });
        }
    }

    let dirs: Dirs = match git {
        true => get_repos(&sorted_existing_sessions).context("Error finding all repos")?,
        false => {
            get_directories(&sorted_existing_sessions).context("Error finding all directories")?
        }
    };

    let mut possible_selections: Dirs = Dirs::new();
    possible_selections
        .dirs
        .extend(sorted_existing_sessions.dirs);
    possible_selections.dirs.extend(dirs.dirs);

    let selected =
        select_via_fzf(possible_selections).context("Error selecting new or existing session")?;

    let existing_session = utils::tmux_session_exisits(&selected.name)
        .with_context(|| format!("Error checking if session '{}' exists", selected.name))?;

    if !existing_session {
        create_tmux_session(&selected).context("Error creating new tmux session")?;
    } else {
        utils::tmux_switch_client(&selected.name, Some(1))
            .context("Error switching to existing session")?;
    }

    Ok(())
}

fn get_repos(existing_sessions: &Dirs) -> Result<Dirs> {
    let mut dirs: Dirs = Dirs::new();
    for path in PATHS {
        let child_dirs = PathBuf::from(path)
            .canonicalize()?
            .read_dir()
            .with_context(|| format!("Couldn't get the child directories of {:?}", &path))?;
        for child_dir in child_dirs {
            let dir = child_dir.context("Child directory has an error")?;
            if dir.file_type()?.is_dir() {
                match dir
                    .file_name()
                    .to_str()
                    .context("Error converting filename to str")?
                {
                    // If subfolder named '.git' exists it is a normal git repo
                    ".git" => {
                        let mut path = dir.path();
                        path.pop();

                        let dir = Dir {
                            path: Some(path.clone()),
                            name: path.file_name().unwrap().to_string_lossy().to_string(),
                        };
                        if !existing_sessions.dirs.contains(&dir) {
                            dirs.dirs.push(dir);
                        }
                    }
                    // If not check if subfolder is a git worktree
                    _ => {
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
                            let dir = Dir {
                                path: Some(dir.path()),
                                name: format!("{base}/{worktree}"),
                            };
                            if !existing_sessions.dirs.contains(&dir) {
                                dirs.dirs.push(dir);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(dirs)
}

fn get_directories(existing_sessions: &Dirs) -> Result<Dirs> {
    let mut dirs: Dirs = Dirs::new();
    for path in PATHS {
        let path = PathBuf::from(path);
        let dir = Dir {
            path: Some(path.clone()),
            name: path.file_name().unwrap().to_string_lossy().to_string(),
        };
        if !existing_sessions.dirs.contains(&dir) {
            dirs.dirs.push(dir);
        }
    }

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

fn select_via_fzf(possible_selections: Dirs) -> Result<Dir> {
    let mut child = Command::new("fzf")
        .args(["--margin=5%", "--padding=2%", "--border", "--ansi"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to spawn fzf")?;

    let stdin = child.stdin.as_mut().context("Error opening fzf stdin")?;
    for possibility in &possible_selections.dirs {
        if possibility.path.is_none() {
            // Display already open sessions in bold.
            writeln!(stdin, "\x1b[1m{}\x1b[0m", possibility.name)?;
        } else {
            writeln!(stdin, "{}", possibility.name)?;
        }
    }

    let selected = child
        .wait_with_output()
        .context("Error reading fzf stdout")?
        .stdout;
    let selected = String::from_utf8_lossy(&selected);
    let selected = selected.trim();
    let selected = possible_selections
        .dirs
        .iter()
        .find(|d| d.name == selected)
        .context("Selected value isn't part of provided options")?;

    Ok(selected.clone())
}

#[derive(Debug, Clone)]
struct Dirs {
    dirs: Vec<Dir>,
}

impl Dirs {
    fn new() -> Self {
        Dirs { dirs: Vec::new() }
    }
}

#[derive(Debug, Clone)]
struct Dir {
    path: Option<PathBuf>,
    name: String,
}

impl PartialEq for Dir {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
