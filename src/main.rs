//!
//! # Tmux Sessionizer rewritten in Rust.
//!
//! Originally inspired by `ThePrimeagen` and `saccorium`.
//! Rewritten because I wanted some more advanced features,
//! like working with *git worktrees*.

use anyhow::{Context, Result};
use clap_complete::Shell;
use rost_sessionizer::commands::{
    cli::{SearchMode, build_cli, print_completions},
    kill, open, startup,
};

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
            let search_mode = sub_matches
                .get_one::<SearchMode>("search")
                .expect("default ensures there is always a value");
            open::open(search_mode).context("Error while running the open command")?;
        }
        Some(("kill", _sub_matches)) => {
            kill::kill_current_session().context("Error while trying to kill current session")?;
        }
        Some(("kill-all", _sub_matches)) => {
            kill::kill_all_sessions().context("Error while trying to kill all sessions")?;
        }
        Some(("startup", _sub_matches)) => {
            startup::startup().context("Error while starting default tmux session")?;
        }
        None => println!("Generated bash completion script"),
        e => unreachable!("Should be unreachable!: {:?}", e),
    }

    Ok(())
}
