use crate::utils::types::session_name::SessionName;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::{
    env,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    sync::{LazyLock, OnceLock},
};

const DEFAULT_SESSION: &str = "Default";
const DEFAULT_DEPTH: usize = 1;

static CONFIG: OnceLock<Config> = OnceLock::new();
static DEFAULT: LazyLock<Config> = LazyLock::new(Config::default);

#[derive(Debug)]
struct Config {
    default_session: SessionName,
    paths: Vec<PathBuf>,
    depth: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_session: SessionName::new(DEFAULT_SESSION),
            paths: Vec::new(),
            depth: DEFAULT_DEPTH,
        }
    }
}

#[derive(Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    default_session: Option<String>,
    paths: Option<Vec<PathBuf>>,
    depth: Option<usize>,
}

#[derive(Debug, Default)]
struct Env {
    default_session: Option<OsString>,
    paths: Option<OsString>,
    depth: Option<OsString>,
}

impl Env {
    fn read() -> Self {
        let var = |key: &str| env::var_os(key).filter(|value| !value.is_empty());
        Self {
            default_session: var("DEFAULT_SESSION"),
            paths: var("SESSIONIZER_PATHS"),
            depth: var("SESSIONIZER_DEPTH"),
        }
    }
}

/// Reads the config file and the environment variables.
///
/// # Errors
///
/// Returns an error if the config file can't be read or parsed, or if `SESSIONIZER_DEPTH` isn't a
/// number.
pub fn init() -> Result<()> {
    let home = dirs::home_dir().context("Couldn't determine the home directory")?;
    let path = config_path(dirs::config_dir())?;
    let file = match fs::read_to_string(&path) {
        Ok(text) => parse(&text).with_context(|| format!("Error parsing {}", path.display()))?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => File::default(),
        Err(error) => {
            return Err(error).with_context(|| format!("Error reading {}", path.display()));
        }
    };
    let _ = CONFIG.set(resolve(file, Env::read(), &home)?);

    Ok(())
}

pub(crate) fn default_session() -> SessionName {
    get().default_session.clone()
}

pub(crate) fn paths() -> &'static [PathBuf] {
    &get().paths
}

pub(crate) fn depth() -> usize {
    get().depth
}

fn get() -> &'static Config {
    CONFIG.get().unwrap_or(&DEFAULT)
}

fn config_path(config_dir: Option<PathBuf>) -> Result<PathBuf> {
    Ok(config_dir
        .context("Config directory not set")?
        .join("rost_sessionizer")
        .join("config.toml"))
}

fn parse(text: &str) -> Result<File> {
    Ok(toml::from_str(text)?)
}

fn resolve(file: File, env: Env, home: &Path) -> Result<Config> {
    let default_session = env
        .default_session
        .map(|name| name.to_string_lossy().into_owned())
        .or(file.default_session)
        .unwrap_or_else(|| DEFAULT_SESSION.to_owned());
    let paths = env
        .paths
        .map(|paths| env::split_paths(&paths).collect())
        .or(file.paths)
        .unwrap_or_default();
    let depth = match env.depth {
        Some(raw) => {
            let raw = raw.to_string_lossy();
            raw.parse().with_context(|| {
                format!("SESSIONIZER_DEPTH must be a whole number of at least 0, not '{raw}'")
            })?
        }
        None => file.depth.unwrap_or(DEFAULT_DEPTH),
    };

    Ok(Config {
        default_session: SessionName::new(&default_session),
        paths: paths
            .into_iter()
            .map(|path| expand_tilde(path, home))
            .collect(),
        depth,
    })
}

fn expand_tilde(path: PathBuf, home: &Path) -> PathBuf {
    match path.strip_prefix("~") {
        Ok(rest) => home.join(rest),
        Err(_) => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use test_strategy::proptest;

    const HOME: &str = "/home/u";

    mod parse {
        use super::*;

        #[test]
        fn reads_every_key() {
            let file = parse(
                r#"
                default_session = "home"
                paths = ["~", "/srv/code"]
                depth = 3
                "#,
            )
            .unwrap();
            assert_eq!(
                file,
                File {
                    default_session: Some("home".to_owned()),
                    paths: Some(vec!["~".into(), "/srv/code".into()]),
                    depth: Some(3),
                }
            );
        }

        #[test]
        fn empty_file_sets_nothing() {
            assert_eq!(parse("").unwrap(), File::default());
        }

        #[test]
        fn rejects_unknown_keys() {
            assert!(parse(r#"path = ["~"]"#).is_err());
        }

        #[test]
        fn rejects_wrong_types() {
            for text in [
                "depth = -1",
                r#"depth = "3""#,
                r#"paths = "~""#,
                "default_session = 1",
            ] {
                assert!(parse(text).is_err(), "{text}");
            }
        }
    }

    mod resolve {
        use super::*;

        #[test]
        fn defaults_without_file_or_env() {
            let config = resolve(File::default(), Env::default(), Path::new(HOME)).unwrap();
            assert_eq!(config.default_session, SessionName::new("Default"));
            assert_eq!(config.paths, [] as [PathBuf; 0]);
            assert_eq!(config.depth, 1);
        }

        #[test]
        fn file_replaces_defaults() {
            let file = File {
                default_session: Some("home".to_owned()),
                paths: Some(vec!["/srv/code".into()]),
                depth: Some(3),
            };
            let config = resolve(file, Env::default(), Path::new(HOME)).unwrap();
            assert_eq!(config.default_session, SessionName::new("home"));
            assert_eq!(config.paths, [PathBuf::from("/srv/code")]);
            assert_eq!(config.depth, 3);
        }

        #[test]
        fn env_replaces_file() {
            let file = File {
                default_session: Some("home".to_owned()),
                paths: Some(vec!["/srv/code".into()]),
                depth: Some(3),
            };
            let env = Env {
                default_session: Some("main".into()),
                paths: Some("/a:/b".into()),
                depth: Some("0".into()),
            };
            let config = resolve(file, env, Path::new(HOME)).unwrap();
            assert_eq!(config.default_session, SessionName::new("main"));
            assert_eq!(config.paths, [PathBuf::from("/a"), PathBuf::from("/b")]);
            assert_eq!(config.depth, 0);
        }

        #[test]
        fn keeps_empty_path_entries_for_warnings() {
            let env = Env {
                paths: Some(":/a".into()),
                ..Env::default()
            };
            let config = resolve(File::default(), env, Path::new(HOME)).unwrap();
            assert_eq!(config.paths, [PathBuf::new(), PathBuf::from("/a")]);
        }

        #[test]
        fn expands_tilde_in_file() {
            let file = File {
                paths: Some(vec![
                    "~".into(),
                    "~/code".into(),
                    "~other/code".into(),
                    "/srv/~".into(),
                ]),
                ..File::default()
            };
            let config = resolve(file, Env::default(), Path::new(HOME)).unwrap();
            assert_eq!(
                config.paths,
                [
                    PathBuf::from("/home/u"),
                    PathBuf::from("/home/u/code"),
                    PathBuf::from("~other/code"),
                    PathBuf::from("/srv/~"),
                ]
            );
        }

        #[test]
        fn expands_tilde_in_env() {
            let env = Env {
                paths: Some("~/a:/b".into()),
                ..Env::default()
            };
            let config = resolve(File::default(), env, Path::new(HOME)).unwrap();
            assert_eq!(
                config.paths,
                [PathBuf::from("/home/u/a"), PathBuf::from("/b")]
            );
        }

        #[test]
        fn rejects_invalid_depth() {
            let env = Env {
                depth: Some("deep".into()),
                ..Env::default()
            };
            let error = resolve(File::default(), env, Path::new(HOME)).unwrap_err();
            assert!(format!("{error:#}").contains("SESSIONIZER_DEPTH"));
        }

        #[proptest]
        fn env_wins_over_file_wins_over_default(
            #[any] file_depth: Option<u8>,
            #[any] env_depth: Option<u8>,
        ) {
            let file = File {
                depth: file_depth.map(usize::from),
                ..File::default()
            };
            let env = Env {
                depth: env_depth.map(|depth| depth.to_string().into()),
                ..Env::default()
            };
            let config = resolve(file, env, Path::new(HOME)).unwrap();
            prop_assert_eq!(
                config.depth,
                usize::from(env_depth.or(file_depth).unwrap_or(1))
            );
        }
    }
}
