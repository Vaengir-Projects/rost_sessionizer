# WIP:

# rost_sessionizer
Cli-tool which integrates with tmux to manage sessions based on project folders. It is intended to work well with git worktrees.

## Usage

### Open Command
This command is used to open a new or an existing session using fzf.
It takes all existing sessions, the frecency entries and the folders, repos and worktrees below the configured paths
into account. After selecting an entry tmux will either switch to the existing session or create a new session with a
default layout.

### Kill Command

### Kill-All Command

### Startup Command
// Show example usage in bashrc

### Command Line Interface
```help
Usage: rost_sessionizer [OPTIONS] [COMMAND]

Commands:
  open      Open a new or switch to an existing session in tmux
  kill      Kill active session
  kill-all  Kill all active sessions
  startup   Start tmux with the default session
  frecency  Work with the frecency data [alias: f]
  config    Work with the configuration
  help      Print this message or the help of the given subcommand(s)

Options:
  -G, --generate <generator>  [possible values: bash, elvish, fish, powershell, zsh]
  -h, --help                  Print help
  -V, --version               Print version
```

## Requirements

- tmux
- fzf 0.63 or newer, because warnings are shown in the fzf footer

## Installation

## Configuration

Settings are read from `config.toml` in the config directory:
- Linux: `$XDG_CONFIG_HOME/rost_sessionizer/config.toml`, otherwise `~/.config/rost_sessionizer/config.toml`
- macOS: `~/Library/Application Support/rost_sessionizer/config.toml`

`rost_sessionizer config example` prints an example to start from.

| Setting           | Environment variable              | Default   | Meaning                                                   |
| ----------------- | --------------------------------- | --------- | --------------------------------------------------------- |
| `default_session` | `DEFAULT_SESSION`                 | `Default` | Session that `startup` creates and `kill` switches to     |
| `paths`           | `SESSIONIZER_PATHS`, split by `:` | none      | Folders to search, `~` stands for the home directory      |
| `depth`           | `SESSIONIZER_DEPTH`               | `1`       | Folder levels searched below each path                    |

Environment variables take precedence over the file. The search stops at repos and skips hidden folders, unless a
hidden folder is configured itself. Entries are named after their folder; names that occur more than once get their
parent folder (`work/api`, `personal/api`), worktrees are named `<repo>/<worktree>`.

- This should include possible keybinds and quirks with the program working in combination with tmux
  - For example: using `tmux neww` doesn't show you errors per default.
