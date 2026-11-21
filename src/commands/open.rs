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
use anyhow::{Context, Result, bail};
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

    let mut possible_selections: Dirs = HashMap::new();

    for session in tmux::existing_sessions().context("Error getting existing sessions")? {
        possible_selections.try_insert_dir(session);
    }

    let home = dirs::home_dir().context("Couldn't determine the home directory")?;
    // Keep the default session selectable even when it isn't running.
    possible_selections.try_insert_dir(Dir::from((default_session, home, None)));

    // Open connection to frecency database
    let mut conn = db::open().context("Error getting database conntection")?;

    let mut warnings = Vec::new();
    let entries = config::paths().unwrap_or_else(|error| {
        warnings.push(format!("{error:#}"));
        Vec::new()
    });
    let mut configured_dirs = Vec::new();
    for entry in entries {
        match Dir::configured(&entry) {
            Ok(dir) => configured_dirs.push(dir),
            Err(error) => warnings.push(format!("{error:#}")),
        }
    }

    let search_modes: Vec<SearchMode> = search_modes.copied().collect();
    let sources: [(SearchMode, &dyn Fn() -> Result<Dirs>); 4] = [
        (SearchMode::Frecency, &|| {
            db::scores(&conn).context("Error getting paths by frecency")
        }),
        (SearchMode::Worktrees, &|| {
            get_worktrees(&configured_dirs).context("Error finding all worktrees")
        }),
        (SearchMode::Repos, &|| Ok(get_repos(&configured_dirs))),
        (SearchMode::Dirs, &|| Ok(get_directories(&configured_dirs))),
    ];
    for (mode, load) in sources {
        if search_modes.contains(&SearchMode::All) || search_modes.contains(&mode) {
            possible_selections.try_extend(load()?);
        }
    }

    let Some(selected) = select_via_fzf(&possible_selections.sort(), &warnings)
        .context("Error selecting new or existing session")?
    else {
        return Ok(());
    };

    let existing_session = tmux::tmux_session_exists(&selected.name)
        .with_context(|| format!("Error checking if session '{}' exists", selected.name))?;

    if existing_session {
        tmux::tmux_switch_client(&selected.name).context("Error switching to existing session")?;
    } else {
        tmux::create_tmux_session(&selected).context("Error creating new tmux session")?;
    }

    db::bump(&conn, &selected).context("Error updating frecency for selected Dir")?;
    db::age(&mut conn).context("Error aging frecency database")?;

    Ok(())
}

fn get_directories(configured_dirs: &[Dir]) -> Dirs {
    let mut dirs = Dirs::new();
    for dir in configured_dirs {
        if !dir.path.join(".git").exists() {
            dirs.try_insert_dir(dir.clone());
        }
    }

    dirs
}

fn get_repos(configured_dirs: &[Dir]) -> Dirs {
    let mut dirs = Dirs::new();
    for dir in configured_dirs {
        if dir.path.join(".git").exists() {
            dirs.try_insert_dir(dir.clone());
        }
    }

    dirs
}

fn get_worktrees(configured_dirs: &[Dir]) -> Result<Dirs> {
    let mut dirs = Dirs::new();
    for configured_dir in configured_dirs {
        let child_dirs = configured_dir.path.read_dir().with_context(|| {
            format!(
                "Couldn't get the child directories of {}",
                configured_dir.path.display()
            )
        })?;
        for child_dir in child_dirs {
            let dir = child_dir.context("Child directory has an error")?;
            if dir.file_type()?.is_dir() {
                if dir.file_name() == ".git" {
                    dirs.try_insert_dir(configured_dir.clone());
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
                            absolute_path,
                            None,
                        )));
                    }
                }
            }
        }
    }

    Ok(dirs)
}

const FZF_NO_MATCH: i32 = 1;
const FZF_INTERRUPTED: i32 = 130;

/// Allows user to select the session they want to open.
///
/// # Errors
///
/// Returns `io::Error` if the fzf or any of the tmux commands fail, or
/// `anyhow::Error` if anything with stdin or stdout fails.
fn select_via_fzf(possible_selections: &[Dir], warnings: &[String]) -> Result<Option<Dir>> {
    let mut command = Command::new("fzf");
    command.args([
        "--margin=5%",
        "--padding=2%",
        "--border",
        "--ansi",
        "--tiebreak=index",
    ]);
    if !warnings.is_empty() {
        command.args(["--footer", &warnings.join("\n")]);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to spawn fzf")?;

    let stdin = child.stdin.as_mut().context("Error opening fzf stdin")?;

    for dir in possible_selections {
        let name = &dir.name;
        if dir.has_session {
            // Display already open sessions in bold.
            writeln!(stdin, "\x1b[1m{name}\x1b[0m")?;
        } else {
            writeln!(stdin, "{name}")?;
        }
    }

    let output = child
        .wait_with_output()
        .context("Error reading fzf stdout")?;
    match output.status.code() {
        Some(0) => {}
        Some(FZF_NO_MATCH | FZF_INTERRUPTED) => return Ok(None),
        _ => bail!("fzf failed with {}", output.status),
    }

    let selected = String::from_utf8_lossy(&output.stdout);
    let selected = selected.trim();
    possible_selections
        .iter()
        .find(|dir| dir.name.as_str() == selected)
        .cloned()
        .context("Selected value isn't part of provided options")
        .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        path::{Path, PathBuf},
    };
    use tempfile::TempDir;
    use test_strategy::{Arbitrary, proptest};

    #[derive(Debug, Clone, Copy, Arbitrary)]
    enum Kind {
        Plain,
        RepoDir,
        RepoFile,
    }

    impl Kind {
        fn is_repo(self) -> bool {
            !matches!(self, Self::Plain)
        }
    }

    fn create(path: &Path, kind: Kind) {
        fs::create_dir_all(path).unwrap();
        match kind {
            Kind::Plain => {}
            Kind::RepoDir => fs::create_dir(path.join(".git")).unwrap(),
            Kind::RepoFile => fs::write(path.join(".git"), "gitdir: elsewhere").unwrap(),
        }
    }

    fn create_all(parent: &Path, entries: &BTreeMap<String, Kind>) -> Vec<PathBuf> {
        entries
            .iter()
            .map(|(name, kind)| {
                let path = parent.join(name);
                create(&path, *kind);
                path
            })
            .collect()
    }

    mod directories_and_repos {
        use super::*;

        #[test]
        fn dot_git_decides_between_directory_and_repo() {
            let tmp = TempDir::new().unwrap();
            let plain = tmp.path().join("plain");
            let repo = tmp.path().join("repo");
            let worktree = tmp.path().join("worktree");
            create(&plain, Kind::Plain);
            create(&repo, Kind::RepoDir);
            create(&worktree, Kind::RepoFile);
            let configured_dirs =
                [plain, repo, worktree].map(|path| Dir::configured(&path).unwrap());

            assert_eq!(
                get_directories(&configured_dirs)
                    .keys()
                    .map(ToString::to_string)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["plain".to_owned()])
            );
            assert_eq!(
                get_repos(&configured_dirs)
                    .keys()
                    .map(ToString::to_string)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["repo".to_owned(), "worktree".to_owned()])
            );
        }

        #[test]
        fn first_path_wins_name() {
            let tmp = TempDir::new().unwrap();
            let work = tmp.path().join("work/api");
            let personal = tmp.path().join("personal/api");
            create(&work, Kind::Plain);
            create(&personal, Kind::Plain);

            let configured_dirs = [&work, &personal].map(|path| Dir::configured(path).unwrap());
            let found = get_directories(&configured_dirs);
            assert_eq!(found.len(), 1);
            assert_eq!(
                found[&SessionName::new("api")].path,
                work.canonicalize().unwrap()
            );
        }

        #[proptest(cases = 32)]
        fn every_path_is_directory_or_repo(
            #[strategy(prop::collection::btree_map("[a-z]{1,8}", any::<Kind>(), 0..8))]
            entries: BTreeMap<String, Kind>,
        ) {
            let tmp = TempDir::new().unwrap();
            let configured_dirs: Vec<Dir> = create_all(tmp.path(), &entries)
                .iter()
                .map(|path| Dir::configured(path).unwrap())
                .collect();
            let plain: BTreeSet<String> = entries
                .iter()
                .filter(|(_, kind)| !kind.is_repo())
                .map(|(name, _)| name.clone())
                .collect();
            let repos: BTreeSet<String> = entries
                .iter()
                .filter(|(_, kind)| kind.is_repo())
                .map(|(name, _)| name.clone())
                .collect();
            prop_assert_eq!(
                get_directories(&configured_dirs)
                    .keys()
                    .map(ToString::to_string)
                    .collect::<BTreeSet<_>>(),
                plain
            );
            prop_assert_eq!(
                get_repos(&configured_dirs)
                    .keys()
                    .map(ToString::to_string)
                    .collect::<BTreeSet<_>>(),
                repos
            );
        }
    }

    mod worktrees {
        use super::*;

        #[test]
        fn bare_repository() {
            let tmp = TempDir::new().unwrap();
            let project = tmp.path().join("project");
            create(&project, Kind::RepoDir);
            create(&project.join("main"), Kind::RepoFile);
            create(&project.join("feature"), Kind::RepoFile);
            create(&project.join("notes"), Kind::Plain);
            fs::write(project.join("readme"), "").unwrap();

            let found = get_worktrees(&[Dir::configured(&project).unwrap()]).unwrap();
            assert_eq!(
                found
                    .keys()
                    .map(ToString::to_string)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from([
                    "project".to_owned(),
                    "project/feature".to_owned(),
                    "project/main".to_owned()
                ])
            );
            assert_eq!(
                found[&SessionName::new("project/main")].path,
                project.join("main").canonicalize().unwrap()
            );
        }

        #[test]
        #[cfg(target_os = "linux")]
        fn child_with_non_utf8_name_does_not_break() {
            use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

            let tmp = TempDir::new().unwrap();
            let project = tmp.path().join("project");
            create(&project, Kind::RepoDir);
            create(&project.join(OsStr::from_bytes(b"caf\xe9")), Kind::Plain);

            let found = get_worktrees(&[Dir::configured(&project).unwrap()]).unwrap();
            assert!(found.contains_key(&SessionName::new("project")));
        }

        #[proptest(cases = 32)]
        fn children_with_dot_git(
            #[any] project_kind: Kind,
            #[strategy(prop::collection::btree_map("[a-z]{1,8}", any::<Kind>(), 0..8))]
            children: BTreeMap<String, Kind>,
        ) {
            let tmp = TempDir::new().unwrap();
            let project = tmp.path().join("project");
            create(&project, project_kind);
            create_all(&project, &children);

            let mut expected: BTreeSet<_> = children
                .iter()
                .filter(|(_, kind)| kind.is_repo())
                .map(|(name, _)| format!("project/{name}"))
                .collect();
            if matches!(project_kind, Kind::RepoDir) {
                expected.insert("project".to_owned());
            }
            prop_assert_eq!(
                get_worktrees(&[Dir::configured(&project).unwrap()])
                    .unwrap()
                    .keys()
                    .map(ToString::to_string)
                    .collect::<BTreeSet<_>>(),
                expected
            );
        }
    }
}
