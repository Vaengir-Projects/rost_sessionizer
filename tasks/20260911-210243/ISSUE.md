# WIP:

# rost_sessionizer
Cli-tool which integrates with tmux to manage sessions based on project folders. It is intended to work well with git worktrees.

## Usage

### Open command
This command is used to open a new new or an existing session using fzf.
// TODO: #10 adjust documentation after configurable <2025-07-13>
It takes all existing sessions the configured directories and worktrees inside those directories into account.
After selecting an entry tmux will either switch to the existing session or create a new session with a default layout.

### Kill command

### Kill-all command

### Startup command
// Show example usage in bashrc

### Command line interface
```help
Usage: rost_sessionizer [OPTIONS] [COMMAND]

Commands:
  open      Open a new or switch to an existing session in tmux
  kill      Kill active session
  kill-all  Kill all active sessions
  startup   Start tmux with the default session
  help      Print this message or the help of the given subcommand(s)

Options:
  -G, --generate <generator>  [possible values: bash, elvish, fish, powershell, zsh]
  -h, --help                  Print help
  -V, --version               Print version
```

## Installation

## Configuration
