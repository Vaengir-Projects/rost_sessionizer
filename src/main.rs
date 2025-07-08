//! Tmux Sessionizer rewritten in Rust.
//! Originally inspired by ThePrimeagen and saccorium.
//! Rewritten because I wanted some more advanced features,
//! like working with *git worktrees*.

use anyhow::{Context, Result};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

// TODO: #2 Add different arguments to allow closing of sessions <2025-07-08>
fn main() -> Result<()> {
    // TODO: #1 Make configurable <2025-07-08>
    let paths: Vec<PathBuf> = vec![
        PathBuf::from("/home/vaengir/personal/Bachelor_Latex/"),
        PathBuf::from("/home/vaengir/vaengir/AwesomeWM/"),
        PathBuf::from("/home/vaengir/vaengir/CLearn/"),
        PathBuf::from("/home/vaengir/vaengir/Neovim/"),
        PathBuf::from("/home/vaengir/vaengir/Scripts/"),
        PathBuf::from("/home/vaengir/vaengir/ZigLearn/"),
        PathBuf::from("/home/vaengir/vaengir/dotfiles/"),
        PathBuf::from("/home/vaengir/vaengir/harpoon/"),
        PathBuf::from("/home/vaengir/vaengir/quicker.nvim//"),
        PathBuf::from("/home/vaengir/vaengir/rost_interpreter/"),
        PathBuf::from("/home/vaengir/vaengir/rost_sessionizer/"),
        PathBuf::from("/home/vaengir/vaengir/symbols-outline.nvim//"),
        PathBuf::from("/home/vaengir/vaengir/zig_compiler/"),
        PathBuf::from("/home/vaengir/vaengir/rigit/"),
    ];

    // TODO: #3 Filter out dirs that have open session <2025-07-08>
    let mut dirs: Dirs = Dirs::new();
    for mut path in paths {
        path = path.canonicalize()?;
        let child_dirs = path
            .read_dir()
            .with_context(|| format!("Couldn't get the child directories of {:?}", &path))?;
        for child_dir in child_dirs {
            let dir = child_dir.context("Child directory has an error")?;
            if dir.file_type()?.is_dir() {
                if dir.file_name().eq(".git") {
                    let mut path = dir.path();
                    path.pop();

                    dirs.dirs.push(Dir {
                        path: path.clone(),
                        name: path.file_name().unwrap().to_string_lossy().to_string(),
                    });
                    break;
                }
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

                    dirs.dirs.push(Dir {
                        path: dir.path(),
                        name: format!("{base}/{worktree}"),
                    });
                }
            }
        }
    }

    let existing_sessions = Command::new("tmux")
        .arg("ls")
        .output()
        .context("Error listing existing tmux sessions")?
        .stdout;
    let existing_sessions = String::from_utf8_lossy(&existing_sessions);
    let existing_sessions: Vec<String> = existing_sessions.lines().map(|s| s.to_string()).collect();

    let mut sorted_existing_sessions: Dirs = Dirs {
        dirs: vec![Dir {
            name: String::from("Default"),
            path: PathBuf::new(),
        }],
    };
    for existing_session in existing_sessions {
        let session_name: &str = existing_session.split(':').collect::<Vec<&str>>()[0];
        if session_name != "Default" {
            sorted_existing_sessions.dirs.push(Dir {
                name: session_name.to_string(),
                path: PathBuf::new(),
            });
        }
    }

    let mut child = Command::new("fzf")
        .args(["--margin=5%", "--padding=2%", "--border", "--ansi"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to fzf over relevant firs")?;

    let stdin = child.stdin.as_mut().context("Error opening fzf stdin")?;
    for existing_session in &sorted_existing_sessions.dirs {
        // Display already open sessions in bold.
        writeln!(stdin, "\x1b[1m{}\x1b[0m", existing_session.name)?;
    }
    for dir in &dirs.dirs {
        writeln!(stdin, "{}", dir.name)?;
    }

    let selected = child
        .wait_with_output()
        .context("Error reading fzf stdout")?
        .stdout;
    let selected = String::from_utf8_lossy(&selected);
    let selected = selected.trim();
    let mut possible_selections: Dirs = Dirs::new();
    possible_selections
        .dirs
        .extend(sorted_existing_sessions.dirs);
    possible_selections.dirs.extend(dirs.dirs);
    let selected = possible_selections
        .dirs
        .iter()
        .find(|d| d.name == selected)
        .context("Selected value isn't part of provided options")?;

    let existing_session = Command::new("tmux")
        .args(["has-session", "-t", &selected.name])
        .status()
        .map(|status| status.success())
        .context("Error checking if session exists")?;

    if !existing_session {
        // Create Session
        Command::new("tmux")
            .args([
                "new-session",
                "-ds",
                &selected.name,
                "-c",
                &selected.path.to_string_lossy(),
            ])
            .status()
            .context("Error creating tmux session")?;
        // Setup Window Layout
        Command::new("tmux")
            .args([
                "rename-window",
                "-t",
                &format!("{}:1", &selected.name),
                "Neovim",
            ])
            .status()
            .context("Error renaming first window")?;
        Command::new("tmux")
            .args([
                "new-window",
                "-t",
                &format!("{}:2", &selected.name),
                "-c",
                &selected.path.to_string_lossy(),
            ])
            .status()
            .context("Error creating second window")?;
        Command::new("tmux")
            .args([
                "rename-window",
                "-t",
                &format!("{}:2", &selected.name),
                "Bash",
            ])
            .status()
            .context("Error renaming second window")?;
        Command::new("tmux")
            .args(["select-window", "-t", &format!("{}:1", &selected.name)])
            .status()
            .context("Error switching back to first window")?;
        Command::new("tmux")
            .args([
                "send-keys",
                "-t",
                &format!("{}:1", &selected.name),
                "v",
                "Enter",
            ])
            .status()
            .context("Error starting Neovim")?;
    }
    Command::new("tmux")
        .args(["switch-client", "-t", &selected.name])
        .status()
        .context("Error switching to existing session")?;
    Command::new("tmux")
        .args(["select-window", "-t", &format!("{}:1", &selected.name)])
        .status()
        .context("Error switching to first window")?;

    Ok(())
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
    path: PathBuf,
    name: String,
}
