//!
//! # CLI argument handler
//!
//! This module handles the CLI arguments using clap.

use anyhow::{Result, bail};
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
                                .value_parser(validate_name),
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
                    Command::new("modify")
                        .visible_alias("mod")
                        .about("Modify an entry from the frecency database")
                        .arg(
                            Arg::new("old_name")
                                .value_name("old_name")
                                .help("Name of frecency entry to change")
                                .required(true)
                                .value_parser(validate_name),
                        )
                        .arg(
                            Arg::new("new_name")
                                .value_name("new_name")
                                .help("Name the frecency entry should be changed to")
                                .required(true)
                                .value_parser(validate_name),
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
                                .value_parser(validate_name),
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

fn validate_name(name: &str) -> Result<String> {
    if name.trim().is_empty() {
        bail!("Error: name must contain at least one non-whitespace character");
    }
    // This is a limitation by tmux. See #20260911-210534
    if name.chars().find(|c| matches!(c, '.' | ':')).is_some() {
        bail!("Error: name must not contain '.' or ':', use '_' or '_' instead");
    }

    Ok(name.to_owned())
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

#[derive(Debug, Clone, ValueEnum)]
pub enum SearchMode {
    All,
    Dirs,
    Repos,
    Worktrees,
    Frecency,
}
