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
    cell::OnceCell,
    collections::{HashMap, HashSet},
    io::Write,
    path::Path,
    process::{Command, Stdio},
};
use walkdir::WalkDir;

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
    if config::paths().is_empty() {
        let file = config::file_path().map_or_else(
            |_| "the config file".to_owned(),
            |path| path.display().to_string(),
        );
        warnings.push(format!(
            "Configuration needed: set SESSIONIZER_PATHS or paths in {file}\n\
            `rost_sessionizer config example` prints an example config file"
        ));
    }
    let mut configured_dirs = Vec::new();
    for entry in config::paths() {
        match Dir::configured(entry) {
            Ok(dir) => configured_dirs.push(dir),
            Err(error) => warnings.push(format!("{error:#}")),
        }
    }
    let discovered = OnceCell::new();
    let discovered_dirs = || discovered.get_or_init(|| discover(&configured_dirs, config::depth()));

    let search_modes: Vec<SearchMode> = search_modes.copied().collect();
    let sources: [(SearchMode, &dyn Fn() -> Result<Dirs>); 4] = [
        (SearchMode::Frecency, &|| {
            db::scores(&conn).context("Error getting paths by frecency")
        }),
        (SearchMode::Worktrees, &|| {
            get_worktrees(discovered_dirs()).context("Error finding all worktrees")
        }),
        (SearchMode::Repos, &|| Ok(get_repos(discovered_dirs()))),
        (SearchMode::Dirs, &|| Ok(get_directories(discovered_dirs()))),
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

fn discover(configured_dirs: &[Dir], depth: usize) -> Vec<Dir> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();
    for configured_dir in configured_dirs {
        let mut walk = WalkDir::new(&configured_dir.path)
            .max_depth(depth)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|entry| {
                entry.depth() == 0 || !entry.file_name().to_string_lossy().starts_with('.')
            });
        while let Some(entry) = walk.next() {
            let Ok(entry) = entry else {
                continue;
            };
            if !entry.file_type().is_dir() {
                continue;
            }
            if entry.path().join(".git").exists() {
                walk.skip_current_dir();
            }
            if seen.insert(entry.path().to_path_buf()) {
                paths.push(entry.into_path());
            }
        }
    }

    let folder_name =
        |path: &Path| SessionName::new(&path.file_name().unwrap_or_default().to_string_lossy());
    let mut counts: HashMap<SessionName, usize> = HashMap::new();
    for path in &paths {
        *counts.entry(folder_name(path)).or_default() += 1;
    }

    paths
        .into_iter()
        .map(|path| {
            let name = folder_name(&path);
            let name = match path.parent().and_then(Path::file_name) {
                Some(parent) if counts[&name] > 1 => {
                    SessionName::new(&format!("{}/{name}", parent.to_string_lossy()))
                }
                _ => name,
            };
            Dir::from((name, path, None))
        })
        .collect()
}

fn get_directories(discovered_dirs: &[Dir]) -> Dirs {
    let mut dirs = Dirs::new();
    for dir in discovered_dirs {
        if !dir.path.join(".git").exists() {
            dirs.try_insert_dir(dir.clone());
        }
    }

    dirs
}

fn get_repos(discovered_dirs: &[Dir]) -> Dirs {
    let mut dirs = Dirs::new();
    for dir in discovered_dirs {
        if dir.path.join(".git").exists() {
            dirs.try_insert_dir(dir.clone());
        }
    }

    dirs
}

fn get_worktrees(discovered_dirs: &[Dir]) -> Result<Dirs> {
    let mut dirs = Dirs::new();
    for repo in discovered_dirs {
        if !repo.path.join(".git").exists() {
            continue;
        }
        let child_dirs = repo.path.read_dir().with_context(|| {
            format!(
                "Couldn't get the child directories of {}",
                repo.path.display()
            )
        })?;
        for child_dir in child_dirs {
            let dir = child_dir.context("Child directory has an error")?;
            if dir.file_type()?.is_dir() {
                if dir.file_name() == ".git" {
                    dirs.try_insert_dir(repo.clone());
                } else if dir.path().join(".git").try_exists()? {
                    let name = format!(
                        "{}/{}",
                        repo.path.file_name().unwrap_or_default().to_string_lossy(),
                        dir.file_name().to_string_lossy()
                    );
                    dirs.try_insert_dir(Dir::from((SessionName::new(&name), dir.path(), None)));
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

        #[test]
        fn names_are_repo_and_worktrees() {
            let tmp = TempDir::new().unwrap();
            let home = tmp.path().join("home");
            create(&home.join("code/project"), Kind::RepoDir);
            create(&home.join("code/project/main"), Kind::RepoFile);

            let found = get_worktrees(&discover(&[Dir::configured(&home).unwrap()], 2)).unwrap();
            assert_eq!(
                found
                    .keys()
                    .map(ToString::to_string)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["project", "project/main"].map(String::from))
            );
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
                .filter(|(_, kind)| project_kind.is_repo() && kind.is_repo())
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

    mod discover {
        use super::*;

        #[test]
        fn depth_zero_finds_only_the_configured_folder() {
            let tmp = TempDir::new().unwrap();
            let code = tmp.path().join("code");
            create(&code.join("api"), Kind::RepoDir);

            let found = discover(&[Dir::configured(&code).unwrap()], 0);
            assert_eq!(
                found
                    .iter()
                    .map(|dir| dir.name.to_string())
                    .collect::<Vec<_>>(),
                ["code"]
            );
        }

        #[test]
        fn names_are_folder_names() {
            let tmp = TempDir::new().unwrap();
            let code = tmp.path().join("code");
            create(&code.join("work/api"), Kind::RepoDir);
            create(&code.join("notes"), Kind::Plain);
            fs::write(code.join("readme"), "").unwrap();

            let found = discover(&[Dir::configured(&code).unwrap()], 2);
            assert_eq!(
                found
                    .iter()
                    .map(|dir| dir.name.to_string())
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["code", "notes", "work", "api"].map(String::from))
            );
            let api = found.iter().find(|dir| dir.name.as_str() == "api").unwrap();
            assert_eq!(api.path, code.join("work/api").canonicalize().unwrap());
        }

        #[test]
        fn stops_at_depth() {
            let tmp = TempDir::new().unwrap();
            let code = tmp.path().join("code");
            create(&code.join("a/b/c"), Kind::Plain);

            let found = discover(&[Dir::configured(&code).unwrap()], 2);
            assert_eq!(
                found
                    .iter()
                    .map(|dir| dir.name.to_string())
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["code", "a", "b"].map(String::from))
            );
        }

        #[test]
        fn does_not_look_inside_repos() {
            let tmp = TempDir::new().unwrap();
            let code = tmp.path().join("code");
            create(&code.join("api"), Kind::RepoDir);
            create(&code.join("api/src"), Kind::Plain);
            create(&code.join("api/vendor/lib"), Kind::RepoDir);
            create(&code.join("worktree"), Kind::RepoFile);
            create(&code.join("worktree/src"), Kind::Plain);

            let found = discover(&[Dir::configured(&code).unwrap()], 3);
            assert_eq!(
                found
                    .iter()
                    .map(|dir| dir.name.to_string())
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["code", "api", "worktree"].map(String::from))
            );
        }

        #[test]
        fn skips_hidden_subfolders() {
            let tmp = TempDir::new().unwrap();
            let code = tmp.path().join("code");
            create(&code.join(".cache/x"), Kind::Plain);
            create(&code.join("visible"), Kind::Plain);

            let found = discover(&[Dir::configured(&code).unwrap()], 2);
            assert_eq!(
                found
                    .iter()
                    .map(|dir| dir.name.to_string())
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["code", "visible"].map(String::from))
            );
        }

        #[test]
        fn hidden_folders_need_their_own_entry() {
            let tmp = TempDir::new().unwrap();
            let home = tmp.path().join("home");
            create(&home.join(".config/nvim"), Kind::Plain);

            let only_home = discover(&[Dir::configured(&home).unwrap()], 2);
            assert!(
                !only_home
                    .iter()
                    .any(|dir| dir.path.ends_with(".config/nvim"))
            );

            let with_dot_config = discover(
                &[
                    Dir::configured(&home).unwrap(),
                    Dir::configured(&home.join(".config")).unwrap(),
                ],
                2,
            );
            assert_eq!(
                with_dot_config
                    .iter()
                    .map(|dir| dir.name.to_string())
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["home", "_config", "nvim"].map(String::from))
            );
        }

        #[test]
        fn doubled_names_get_their_parent() {
            let tmp = TempDir::new().unwrap();
            let work = tmp.path().join("work");
            let personal = tmp.path().join("personal");
            create(&work.join("api"), Kind::Plain);
            create(&personal.join("api"), Kind::Plain);
            create(&personal.join("notes"), Kind::Plain);

            let found = discover(
                &[
                    Dir::configured(&work).unwrap(),
                    Dir::configured(&personal).unwrap(),
                ],
                1,
            );
            assert_eq!(
                found
                    .iter()
                    .map(|dir| dir.name.to_string())
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(
                    ["work", "personal", "work/api", "personal/api", "notes"].map(String::from)
                )
            );
            assert_eq!(
                found
                    .iter()
                    .find(|dir| dir.name.as_str() == "personal/api")
                    .unwrap()
                    .path,
                personal.join("api").canonicalize().unwrap()
            );
        }

        #[test]
        fn at_most_one_parent_is_added() {
            let tmp = TempDir::new().unwrap();
            let code = tmp.path().join("code");
            create(&code.join("a/x/api"), Kind::Plain);
            create(&code.join("b/x/api"), Kind::Plain);

            let found = discover(&[Dir::configured(&code).unwrap()], 3);
            assert_eq!(
                found
                    .iter()
                    .filter(|dir| dir.path.ends_with("api"))
                    .map(|dir| dir.name.to_string())
                    .collect::<Vec<_>>(),
                ["x/api", "x/api"]
            );
        }

        #[test]
        fn folder_found_twice_is_not_doubled() {
            let tmp = TempDir::new().unwrap();
            let home = tmp.path().join("home");
            create(&home.join("code/api"), Kind::Plain);

            let found = discover(
                &[
                    Dir::configured(&home).unwrap(),
                    Dir::configured(&home.join("code")).unwrap(),
                ],
                2,
            );
            assert_eq!(
                found
                    .iter()
                    .map(|dir| dir.name.to_string())
                    .collect::<Vec<_>>(),
                ["home", "code", "api"]
            );
        }

        #[test]
        #[cfg(unix)]
        fn does_not_follow_symlinks() {
            let tmp = TempDir::new().unwrap();
            let code = tmp.path().join("code");
            create(&code, Kind::Plain);
            create(&tmp.path().join("elsewhere/inner"), Kind::Plain);
            std::os::unix::fs::symlink(tmp.path().join("elsewhere"), code.join("link")).unwrap();

            let found = discover(&[Dir::configured(&code).unwrap()], 2);
            assert_eq!(
                found
                    .iter()
                    .map(|dir| dir.name.to_string())
                    .collect::<Vec<_>>(),
                ["code"]
            );
        }

        #[proptest(cases = 32)]
        fn found_folders_stay_within_depth_and_outside_repos(
            #[strategy(prop::collection::btree_map("[a-c]{1,2}(/[a-c]{1,2}){0,3}", any::<Kind>(), 0..12))]
            tree: BTreeMap<String, Kind>,
            #[strategy(0..5usize)] depth: usize,
        ) {
            let tmp = TempDir::new().unwrap();
            let code = tmp.path().join("code");
            create(&code, Kind::Plain);
            for (path, kind) in &tree {
                create(&code.join(path), *kind);
            }

            let code_path = code.canonicalize().unwrap();
            for dir in discover(&[Dir::configured(&code).unwrap()], depth) {
                let relative = dir.path.strip_prefix(&code_path).unwrap();
                prop_assert!(relative.components().count() <= depth);
                for ancestor in dir
                    .path
                    .ancestors()
                    .skip(1)
                    .take_while(|ancestor| ancestor.starts_with(&code_path))
                {
                    prop_assert!(
                        !ancestor.join(".git").exists(),
                        "{:?} is inside a repo",
                        dir.path
                    );
                }
            }
        }

        #[proptest(cases = 32)]
        fn plain_names_are_unique(
            #[strategy(prop::collection::btree_map("[a-c]{1,2}(/[a-c]{1,2}){0,3}", any::<Kind>(), 0..12))]
            tree: BTreeMap<String, Kind>,
            #[strategy(0..5usize)] depth: usize,
        ) {
            let tmp = TempDir::new().unwrap();
            let code = tmp.path().join("code");
            create(&code, Kind::Plain);
            for (path, kind) in &tree {
                create(&code.join(path), *kind);
            }

            let found = discover(&[Dir::configured(&code).unwrap()], depth);
            let mut plain_names = HashSet::new();
            for dir in &found {
                let folder = dir.path.file_name().unwrap().to_string_lossy();
                prop_assert!(dir.name.as_str().ends_with(folder.as_ref()));
                prop_assert!(dir.name.as_str().matches('/').count() <= 1);
                if !dir.name.as_str().contains('/') {
                    prop_assert!(
                        plain_names.insert(dir.name.as_str()),
                        "{} appears twice",
                        dir.name
                    );
                }
            }
        }
    }
}
