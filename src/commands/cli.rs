//!
//! # CLI argument handler
//!
//! This module handles the CLI arguments using clap

use clap::{Arg, ArgAction, Command, crate_version, value_parser};
use clap_complete::{Generator, Shell, generate, generator};
use std::io;

/// Function to create the CLI structure using clap
pub(crate) fn build_cli() -> Command {
    Command::new("rigit")
        .name("rigit")
        .author("Væñgír, <vaengir@gmx.de>")
        .version(crate_version!())
        .about("Cli-tool which integrates with tmux to manage sessions based on project folders. It is intended to work well with git worktrees.")
        .arg_required_else_help(true)
        .subcommand(Command::new("open")
            .about("Open a new or existing session in tmux")
            .arg_required_else_help(true)
            .arg(Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Use verbose output")
                .action(ArgAction::SetTrue)))
        .subcommand(Command::new("kill")
            .about("Kill active session")
            .arg_required_else_help(true)
        .subcommand(Command::new("kill-all")
            .about("Kill all active sessions")
            .arg_required_else_help(true)))
        .arg(
            Arg::new("generator")
                .short('G')
                .long("generate")
                .action(ArgAction::Set)
                .value_parser(value_parser!(Shell)),
        )
}

/// Function to create output for bash completion
pub(crate) fn print_completions<G: Generator>(generator: G, cmd: &mut Command) {
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
