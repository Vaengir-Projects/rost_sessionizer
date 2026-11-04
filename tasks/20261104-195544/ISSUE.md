# L1: `open` Doesn't Check That It Runs Inside Tmux

## Where:

`src/commands/open.rs:27`

Outside tmux, `switch-client` fails (silently, see F2) and `get_client_terminal_size` fails with "Unexpected format
from tmux display-message". There's no clear message.

# L2: Database Busy Timeout

## Where:

`src/db.rs:36-39`

`busy_timeout` lowers rusqlite's default of 5 s to 0.5 s, and it's set only after the WAL pragma. Two `open` runs at
the same moment can fail with "database is locked".

# L3: Completion Output Is Wrong

## Where:

`src/main.rs:45`; `src/commands/cli.rs:130-144`

- `main.rs:45` always prints "Generated bash completion script", whichever shell was chosen.
- `print_completions` writes separator lines and an instruction to stdout around the script, so `rost_sessionizer -G
  zsh > _rost_sessionizer` produces a broken file.

# L4: `-s` Without a Value Searches Nothing

## Where:

`src/commands/cli.rs:33`

`num_args(0..=4)` accepts `open -s` with no value. The list then only contains `Default` and the open sessions.

# L5: `--verbose` Does Nothing

## Where:

`src/main.rs:26`

The flag is parsed into `_verbose` and never used.

# L6: Frecency Subcommands

## Where:

`src/db.rs:53-68`, `:240-245`; `src/commands/frecency.rs:34-37`

- `frecency rm` succeeds without any message when the name doesn't exist.
- `frecency new` with a name that already exists shows the raw SQLite UNIQUE error. `modify` already maps that error to
  a readable message.
- `frecency new` accepts a path to a file.

# L7: Copy-Pasted or Wrong Text

- `frecency()` doc comment says "Kills the session the user is currently attached to." (`commands/frecency.rs:11`)
- `startup()` doc comment says "Allows user to select the session they want to open." (`commands/startup.rs:10`)
- Worktree errors say "Error finding all repos" (`commands/open.rs:58`, `:72`).
- `validate_name` says "use '_' or '_' instead" (`utils/mod.rs:51`).
- Typos: `tmux_session_exisits`, "conntection" (several places), `now()` context "Error getting" (`db.rs:17`).
- `tmux_command_without_output`'s error message has an unbalanced backtick and pretty-prints the args across several
  lines (`utils/tmux.rs:14`).

# L8: Duplicated or Wasted Work

- `get_directories` and `get_repos` are identical apart from the `.git` check (`commands/open.rs:102-150`).
- In `all` mode, the configured paths are read and canonicalized three times per `open`.
- The score SQL `CASE` is written twice, in `list` and in `scores` (`db.rs:111-121`, `:291-300`).
- `existing_session_names()` runs twice per `open` (`commands/open.rs:28`, `:242`).
- `config::default_session()` runs in every sort comparison, reading an environment variable and allocating a string
  each time (`commands/open.rs:295`, `:298`).
- `get_client_terminal_size` calls tmux directly instead of going through the wrappers in `tmux.rs`
  (`utils/mod.rs:13-30`).
- `existing_session_names` does `.context(…).unwrap()`, so the context is pointless and an unexpected line panics
  (`utils/tmux.rs:83-84`).
