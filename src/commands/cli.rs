//!
//! # CLI argument handler
//!
//! This module handles the CLI arguments using clap.

use clap::{Arg, ArgAction, Command, ValueEnum, crate_version, value_parser};
use clap_complete::{Generator, Shell, generate};
use std::io;

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
                    Arg::new("git")
                        .short('g')
                        .long("git")
                        .help("Only use configured directories that are git repositories")
                        .num_args(0..=1)
                        .action(ArgAction::Set)
                        .value_parser(value_parser!(GitMode)),
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

#[derive(Debug, Clone, ValueEnum)]
pub enum GitMode {
    Worktrees,
}
