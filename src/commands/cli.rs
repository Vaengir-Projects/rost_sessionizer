//!
//! # CLI argument handler
//!
//! This module handles the CLI arguments using clap.

use crate::utils::types::session_name::SessionName;
use clap::{Arg, ArgAction, Command, ValueEnum, ValueHint, crate_version, value_parser};
use clap_complete::{Generator, Shell, generate};
use std::{io, path::PathBuf};

/// Function to create the CLI structure using clap
#[must_use]
pub fn build_cli() -> Command {
    Command::new("rost_sessionizer")
        .name("rost_sessionizer")
        .author("Væñgír, <vaengir@gmx.de>")
        .version(crate_version!())
        .about(
            "Cli-tool which integrates with tmux to manage sessions based on project folders.
            It is intended to work well with git worktrees.",
        )
        .arg_required_else_help(true)
        .subcommand(
            Command::new("open")
                .about("Open a new or switch to an existing session in tmux")
                .arg(
                    Arg::new("search")
                        .short('s')
                        .long("search")
                        .help("Configure what should be searchable from the given directories")
                        .action(ArgAction::Set)
                        .value_parser(value_parser!(SearchMode))
                        .num_args(0..=4)
                        .default_value("all"),
                )
                .arg(
                    Arg::new("verbose")
                        .short('v')
                        .long("verbose")
                        .help("Use verbose output")
                        .action(ArgAction::SetTrue),
                ),
        )
        .subcommand(Command::new("kill").about("Kill active session"))
        .subcommand(Command::new("kill-all").about("Kill all active sessions"))
        .subcommand(Command::new("startup").about("Start tmux with the default session"))
        .subcommand(
            Command::new("frecency")
                .visible_alias("f")
                .about("Work with the frecency data")
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("new")
                        .visible_alias("n")
                        .about("Add a new directory to the frecency database")
                        .arg(
                            Arg::new("name")
                                .value_name("name")
                                .help("Display name for frecency entry")
                                .required(true)
                                .value_parser(value_parser!(SessionName)),
                        )
                        .arg(
                            Arg::new("path")
                                .value_name("path")
                                .help("Path of the frecency entry")
                                .required(true)
                                .value_parser(value_parser!(PathBuf))
                                .value_hint(ValueHint::DirPath),
                        ),
                )
                .subcommand(
                    Command::new("add")
                        .visible_alias("a")
                        .about("Add the current directory to the frecency database")
                        .arg(
                            Arg::new("name")
                                .value_name("name")
                                .help("Display name for frecency database entry")
                                .value_parser(value_parser!(SessionName))
                                .conflicts_with("worktree"),
                        )
                        .arg(
                            Arg::new("worktree")
                                .short('w')
                                .long("worktree")
                                .help("Use the <parent_dir>/<current_dir> as the name for the new frecency database entry")
                                .action(ArgAction::SetTrue)
                                .conflicts_with("name"),
                        ),
                )
                .subcommand(
                    Command::new("modify")
                        .visible_alias("mod")
                        .about("Modify an entry from the frecency database")
                        .arg(
                            Arg::new("old_name")
                                .value_name("old_name")
                                .help("Name of frecency entry to change")
                                .required(true)
                                .value_parser(value_parser!(SessionName)),
                        )
                        .arg(
                            Arg::new("new_name")
                                .value_name("new_name")
                                .help("Name the frecency entry should be changed to")
                                .required(true)
                                .value_parser(value_parser!(SessionName)),
                        ),
                )
                .subcommand(
                    Command::new("remove")
                        .visible_alias("rm")
                        .about("Remove an entry from the frecency database")
                        .arg(
                            Arg::new("name")
                                .value_name("name")
                                .help("Display name for frecency entry")
                                .required(true)
                                .value_parser(value_parser!(SessionName)),
                        ),
                )
                .subcommand(
                    Command::new("list")
                        .visible_alias("l")
                        .about("List directories in frecency database"),
                ),
        )
        .arg(
            Arg::new("generator")
                .short('G')
                .long("generate")
                .action(ArgAction::Set)
                .value_parser(value_parser!(Shell)),
        )
}

/// Function to create output for bash completion
pub fn print_completions<G: Generator>(generator: G, cmd: &mut Command) {
    println!(
        "-----------------------------------------------------------------------------------------------------"
    );
    generate(
        generator,
        cmd,
        cmd.get_name().to_string(),
        &mut io::stdout(),
    );
    println!(
        "-----------------------------------------------------------------------------------------------------"
    );
    println!("Copy everything between the lines into the corresponding dir for the shell you use.");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SearchMode {
    All,
    Dirs,
    Repos,
    Worktrees,
    Frecency,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{ArgMatches, error::ErrorKind};
    use proptest::prelude::*;
    use test_strategy::proptest;

    fn parse(args: &[&str]) -> Result<ArgMatches, clap::Error> {
        build_cli()
            .try_get_matches_from(std::iter::once("rost_sessionizer").chain(args.iter().copied()))
    }

    #[test]
    fn cli_definition_is_valid() {
        build_cli().debug_assert();
    }

    #[test]
    fn no_args_shows_help() {
        assert_eq!(
            parse(&[]).unwrap_err().kind(),
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
    }

    #[test]
    fn commands_without_args() {
        for command in ["kill", "kill-all", "startup"] {
            let matches = parse(&[command]).unwrap();
            assert_eq!(matches.subcommand_name(), Some(command));
        }
    }

    mod generate {
        use super::*;

        #[test]
        fn accepts_every_shell() {
            for shell in Shell::value_variants() {
                let value = shell.to_possible_value().unwrap();
                let matches = parse(&["-G", value.get_name()]).unwrap();
                assert_eq!(matches.get_one::<Shell>("generator"), Some(shell));
            }
        }

        #[test]
        fn rejects_unknown_shells() {
            assert_eq!(
                parse(&["-G", "cmd"]).unwrap_err().kind(),
                ErrorKind::InvalidValue
            );
        }
    }

    mod open {
        use super::*;

        fn search_modes(args: &[&str]) -> Vec<SearchMode> {
            let matches = parse(args).unwrap();
            let open = matches.subcommand_matches("open").unwrap();

            open.get_many::<SearchMode>("search")
                .unwrap()
                .copied()
                .collect()
        }

        #[test]
        fn search_defaults_to_all() {
            assert_eq!(search_modes(&["open"]), [SearchMode::All]);
        }

        #[test]
        fn search_without_value_is_empty() {
            assert_eq!(search_modes(&["open", "-s"]), []);
        }

        #[test]
        fn search_takes_up_to_four_values() {
            let args = [
                "open",
                "-s",
                "all",
                "dirs",
                "repos",
                "worktrees",
                "frecency",
            ];
            assert!(parse(&args).is_err());
        }

        #[test]
        fn search_rejects_unknown_modes() {
            assert_eq!(
                parse(&["open", "-s", "everything"]).unwrap_err().kind(),
                ErrorKind::InvalidValue
            );
        }

        #[test]
        fn verbose_is_off_unless_given() {
            for (args, expected) in [(vec!["open"], false), (vec!["open", "-v"], true)] {
                let matches = parse(&args).unwrap();
                let open = matches.subcommand_matches("open").unwrap();
                assert_eq!(open.get_flag("verbose"), expected, "{args:?}");
            }
        }

        #[proptest]
        fn search_keeps_given_modes(
            #[strategy(prop::collection::vec(prop::sample::select(SearchMode::value_variants()), 1..=4))]
            modes: Vec<SearchMode>,
        ) {
            let names: Vec<String> = modes
                .iter()
                .map(|mode| mode.to_possible_value().unwrap().get_name().to_owned())
                .collect();
            let mut args = vec!["open", "-s"];
            args.extend(names.iter().map(String::as_str));
            prop_assert_eq!(search_modes(&args), modes);
        }
    }

    mod frecency {
        use super::*;

        #[test]
        fn requires_subcommand() {
            assert_eq!(
                parse(&["frecency"]).unwrap_err().kind(),
                ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
            );
        }

        #[test]
        fn aliases_resolve_to_subcommand() {
            let cases = [
                (vec!["f", "list"], "list"),
                (vec!["frecency", "n", "api", "/tmp"], "new"),
                (vec!["frecency", "a"], "add"),
                (vec!["frecency", "mod", "api", "web"], "modify"),
                (vec!["frecency", "rm", "api"], "remove"),
                (vec!["frecency", "l"], "list"),
            ];
            for (args, expected) in cases {
                let matches = parse(&args).unwrap();
                let frecency = matches.subcommand_matches("frecency").unwrap();
                assert_eq!(frecency.subcommand_name(), Some(expected), "{args:?}");
            }
        }

        #[test]
        fn new_reads_name_and_path() {
            let matches = parse(&["frecency", "new", "api", "/work/api"]).unwrap();
            let new = matches
                .subcommand_matches("frecency")
                .and_then(|m| m.subcommand_matches("new"))
                .unwrap();
            assert_eq!(new.get_one::<SessionName>("name").unwrap().as_str(), "api");
            assert_eq!(
                new.get_one::<PathBuf>("path"),
                Some(&PathBuf::from("/work/api"))
            );
        }

        #[test]
        fn missing_required_args() {
            for args in [
                vec!["frecency", "new", "api"],
                vec!["frecency", "modify", "api"],
                vec!["frecency", "remove"],
            ] {
                assert_eq!(
                    parse(&args).unwrap_err().kind(),
                    ErrorKind::MissingRequiredArgument,
                    "{args:?}"
                );
            }
        }

        #[test]
        fn add_name_conflicts_with_worktree() {
            assert_eq!(
                parse(&["frecency", "add", "api", "-w"]).unwrap_err().kind(),
                ErrorKind::ArgumentConflict
            );
            assert!(parse(&["frecency", "add", "-w"]).is_ok());
            assert!(parse(&["frecency", "add"]).is_ok());
        }

        #[proptest]
        fn every_name_accepts_valid_names(#[strategy("[^.:\\s-][^.:]{0,10}")] name: String) {
            for args in [
                vec!["frecency", "new", &name, "/tmp"],
                vec!["frecency", "add", &name],
                vec!["frecency", "modify", &name, "valid"],
                vec!["frecency", "modify", "valid", &name],
                vec!["frecency", "remove", &name],
            ] {
                prop_assert!(parse(&args).is_ok(), "{args:?}");
            }
        }

        #[proptest]
        fn every_name_rejects_separators(
            #[strategy("([^.:\\s-][^.:]{0,4})?[.:][^.:]{0,5}")] name: String,
        ) {
            for args in [
                vec!["frecency", "new", &name, "/tmp"],
                vec!["frecency", "add", &name],
                vec!["frecency", "modify", &name, "valid"],
                vec!["frecency", "modify", "valid", &name],
                vec!["frecency", "remove", &name],
            ] {
                prop_assert_eq!(
                    parse(&args).unwrap_err().kind(),
                    ErrorKind::ValueValidation,
                    "{:?}",
                    args
                );
            }
        }
    }
}
