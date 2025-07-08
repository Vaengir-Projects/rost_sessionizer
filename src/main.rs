//! Tmux Sessionizer rewritten in Rust.
//! Originally inspired by ThePrimeagen and saccorium.
//! Rewritten because I wanted some more advanced features,
//! like working with *git worktrees*.

use anyhow::{Context, Result};
use clap_complete::Shell;

use crate::commands::{
    cli::{build_cli, print_completions},
    open,
};

// TODO: #4 Clean up code <2025-07-10>
// TODO: #5 Replace tmux resurrect <2025-07-10>
/// Module that handles the logic
pub(crate) mod commands;

fn main() -> Result<()> {
    let args = build_cli().get_matches();

    if let Some(generator) = args.get_one::<Shell>("generator").copied() {
        let mut cmd = build_cli();
        eprintln!("Generating completion file for {generator}...");
        print_completions(generator, &mut cmd);
    }

    match args.subcommand() {
        Some(("open", sub_matches)) => {
            let _verbose = sub_matches.get_flag("verbose");
            open::open().context("Error while running the open command")?;
        }
        Some(("kill", _sub_matches)) => {
            todo!();
        }
        Some(("kill-all", _sub_matches)) => {
            todo!();
        }
        None => println!("Generated bash completion script"),
        e => unreachable!("Should be unreachable!: {:?}", e),
    }

    Ok(())
}
