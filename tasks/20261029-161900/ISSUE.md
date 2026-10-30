# Claude Code review: findings

**Scope:** the whole repository at commit `63621f2` (2026-10-30), about 1,000 lines of Rust in `src/`.
**Method:** I read the code and the git history and checked each finding against the code. Nothing was built or run, and the repository has no tests yet.

## Overview

| ID | Severity | Finding | Where |
|----|----------|---------|-------|
| F1 | High | Names containing `.` or `:` break sessions again (regression) | `commands/open.rs`, `utils/mod.rs` |
| F2 | High | tmux exit codes are never checked | `utils/tmux.rs` |
| F3 | High | `kill` can hit the wrong session | `utils/tmux.rs` |
| F4 | High | The frecency sort is inverted | `commands/open.rs` |
| F5 | High | Entries without a path panic or fail | `utils/tmux.rs`, `commands/open.rs`, `db.rs` |
| F6 | High | Cancelling fzf is reported as an error | `commands/open.rs` |
| F7 | Medium | Session targets match by prefix | `utils/tmux.rs`, `commands/startup.rs` |
| F8 | Medium | Window indices assume `base-index 1` | `utils/tmux.rs`, `commands/startup.rs` |
| F9 | Medium | `kill-all` spares sessions by substring match | `commands/kill.rs` |
| F10 | Medium | Entries with the same name overwrite each other | `utils/mod.rs`, `commands/open.rs` |
| F11 | Medium | `SESSIONIZER_PATHS` isn't validated | `config.rs`, `commands/open.rs` |
| F12 | Medium | The frecency DB keeps stale paths and deletes bookmarks | `db.rs` |
| L1–L8 | Low | Robustness, CLI output, wording, duplicated code | various |

## High

### F1: Names containing `.` or `:` break sessions again (regression)
**Where:** `src/commands/open.rs:81`, `:264`; `src/utils/mod.rs:32-37`

Commit `fa76fbf` (task #20260911-210534) fixed this by sanitizing the keys of the `Dirs` map. At that time `Dirs` was `HashMap<String, Option<PathBuf>>`, so the key *was* the name. Since the frecency work (merged in `3e01933`), the values are `Dir { name, path, rank }` and `select_via_fzf` returns the `Dir`. `sanitize_selections` still only rewrites the keys, so `dir.name` keeps the raw name.

**Failure:** You pick `symbols-outline.nvim`.
- `has-session -t symbols-outline.nvim` reads `.` as a separator and returns false.
- `new-session -s` creates `symbols-outline_nvim`, because tmux sanitizes the name itself.
- `new-window -t symbols-outline.nvim:2` and `switch-client` then fail, and the failure is ignored (F2).
- Every later `open` tries to create the session again.
- `bump` stores the raw name in the DB, and `frecency rm` can't remove it because `validate_name` rejects `.`.

### F2: tmux exit codes are never checked
**Where:** `src/utils/tmux.rs:10-17`

`Command::status()` only returns an error when the tmux binary can't be started, and a non-zero exit code is discarded. Every caller of `tmux_command_without_output` (`switch-client`, `kill-session`, `new-session`, `new-window`, `display-message`, `attach-session`) reports success even when tmux failed. This hides several of the other findings.

**Failure:** `kill` switches to the default session. If that session doesn't exist, the switch fails silently, the current session is killed anyway, and the client drops out of tmux. `open` reports success even when nothing happened.

### F3: `kill` can hit the wrong session
**Where:** `src/utils/tmux.rs:63-74`

`current_session` returns the first line of `tmux ls` that contains "attached". That is the alphabetically first session with *any* client attached, not the session of the client that ran the command. A session whose name contains "attached" also matches.

**Failure:**
- Terminal A is in `Default` and terminal B is in `proj`. Running `kill` in B gets `Default` back: B is switched to `Default` and shown "Can't kill the default session", and `proj` survives.
- With A in `alpha` and B in `zeta`, `kill` in B kills A's session.

### F4: The frecency sort is inverted
**Where:** `src/commands/open.rs:302-305`

`(None, Some(_)) => Less` puts every entry without a rank before every ranked entry. fzf runs with `--tiebreak=index`, so with an empty query the list order is the input order: `Default`, open sessions, every unvisited directory from A to Z, and only then the frecency-ranked entries. The most-used entries end up last. An open session that has a DB entry is replaced by the ranked entry in `try_extend`, so it also loses its "sessions first" place.

### F5: Entries without a path panic or fail
**Where:** `src/utils/tmux.rs:92`; `src/commands/open.rs:96`; `src/db.rs:257-259`

- **a)** `create_tmux_session` calls `unwrap()` on `selected_session.path`. Entries for `Default` and for open sessions have no path, and `open` always lists `Default` first (`open.rs:32-34`), even when it isn't running. Selecting it when it doesn't exist (no `startup` run, or killed by hand) causes a **panic**. The same happens if a listed session closes while fzf is open.
- **b)** `db::bump` requires a path. `try_extend` never gives a session entry the path of the matching directory, because it only compares ranks. So `open -s dirs` followed by picking an open session, or picking a session made by hand (`tmux new -s scratch`), works for the switch. The command then fails with "Error because selection doesn't have a path" and exit code 1, and `db::age` is skipped.

### F6: Cancelling fzf is reported as an error
**Where:** `src/commands/open.rs:253-262`

When you press Esc or Ctrl-C, fzf exits with code 130 and empty stdout. The empty string isn't one of the options, so the tool prints "Error while running the open command: Error selecting new or existing session: Selected value isn't part of provided options" and exits with code 1.

## Medium

### F7: Session targets match by prefix
**Where:** `src/utils/tmux.rs:23`, `:31`, `:43`, `:117`; `src/commands/startup.rs:58`, `:102`

tmux resolves `-t name` by exact match first, then by prefix, then as an fnmatch pattern.

**Failure:** Only `repo/main` is open and you pick `repo`. `has-session -t repo` matches `repo/main` and returns true, so the tool switches into `repo/main` and the `repo` session is never created.

### F8: Window indices assume `base-index 1`
**Where:** `src/utils/tmux.rs:23`, `:117`, `:124`; `src/commands/startup.rs:38`, `:58`, `:102`

The code hard-codes windows `:1` and `:2`. With tmux's default `base-index 0`:
- A new session gets windows 0 (Neovim) and 2 (Bash).
- `switch-client -t name:1` fails with "can't find window: 1", and the failure is ignored (F2), so `open` never moves the client.
- `startup` outside tmux runs `attach-session -t Default:1`, which fails, so the terminal is never attached.

### F9: `kill-all` spares sessions by substring match
**Where:** `src/commands/kill.rs:43`

`retain(|s| !s.contains(&default))` tests for a substring. With `DEFAULT_SESSION=main`, the sessions `domain-api` and `main-old` also survive. A short default name like `a` spares almost every session.

### F10: Entries with the same name overwrite each other
**Where:** `src/utils/mod.rs:32-37`; `src/commands/open.rs:114`, `:139`, `:176`, `:204`

- **a)** `sanitize_selections` collects into a `HashMap`. When two keys sanitize to the same string (directory `foo.bar` and session `foo_bar`), one entry is dropped. Which one survives depends on the HashMap's random iteration order, and the rank preference in `try_extend` is skipped.
- **b)** Directories are keyed by basename with `or_insert`. With `SESSIONIZER_PATHS=~/work/api:~/personal/api`, only the first `api` shows up and `~/personal/api` can't be opened. The same thing happens between `get_directories`, `get_repos` and `get_worktrees` through `try_extend`.

### F11: `SESSIONIZER_PATHS` isn't validated
**Where:** `src/config.rs:10-15`; `src/commands/open.rs:105`, `:111`, `:130`, `:136`, `:155`

- Variable unset: `panic!("No PATHS configured")`.
- Empty segment: the common pattern `SESSIONIZER_PATHS="$SESSIONIZER_PATHS:$HOME/x"`, with the variable empty beforehand, leaves a leading `:`. `canonicalize("")` then fails, and the **whole** `open` aborts, even when you only want an existing session.
- A configured directory that doesn't exist: same as above.
- A configured `/`: `file_name().unwrap()` panics.

### F12: The frecency DB keeps stale paths and deletes bookmarks
**Where:** `src/db.rs:251-254`, `:282`

- **a)** `bump`'s `ON CONFLICT(name) DO UPDATE` never updates `path`, and `try_extend` prefers the ranked DB entry over the entry found on disk. If a repo moves from `~/code/api` to `~/work/api`, the old DB path wins. Sessions are created with the old directory, and `bump` keeps that path until `frecency rm api`.
- **b)** `age()` runs `DELETE … WHERE rank < 1`. Entries created with `frecency new` or `frecency add` start at rank 0. The first time the total rank passes 10,000, every bookmark that was never opened is deleted without notice.

## Low

### L1: `open` doesn't check that it runs inside tmux
**Where:** `src/commands/open.rs:27`

Outside tmux, `switch-client` fails (silently, see F2) and `get_client_terminal_size` fails with "Unexpected format from tmux display-message". There's no clear message.

### L2: Database busy timeout
**Where:** `src/db.rs:36-39`

`busy_timeout` lowers rusqlite's default of 5 s to 0.5 s, and it's set only after the WAL pragma. Two `open` runs at the same moment can fail with "database is locked".

### L3: Completion output is wrong
**Where:** `src/main.rs:45`; `src/commands/cli.rs:130-144`

- `main.rs:45` always prints "Generated bash completion script", whichever shell was chosen.
- `print_completions` writes separator lines and an instruction to stdout around the script, so `rost_sessionizer -G zsh > _rost_sessionizer` produces a broken file.

### L4: `-s` without a value searches nothing
**Where:** `src/commands/cli.rs:33`

`num_args(0..=4)` accepts `open -s` with no value. The list then only contains `Default` and the open sessions.

### L5: `--verbose` does nothing
**Where:** `src/main.rs:26`

The flag is parsed into `_verbose` and never used.

### L6: Frecency subcommands
**Where:** `src/db.rs:53-68`, `:240-245`; `src/commands/frecency.rs:34-37`

- `frecency rm` succeeds without any message when the name doesn't exist.
- `frecency new` with a name that already exists shows the raw SQLite UNIQUE error. `modify` already maps that error to a readable message.
- `frecency new` accepts a path to a file.

### L7: Copy-pasted or wrong text
- `frecency()` doc comment says "Kills the session the user is currently attached to." (`commands/frecency.rs:11`)
- `startup()` doc comment says "Allows user to select the session they want to open." (`commands/startup.rs:10`)
- Worktree errors say "Error finding all repos" (`commands/open.rs:58`, `:72`).
- `validate_name` says "use '_' or '_' instead" (`utils/mod.rs:51`).
- Typos: `tmux_session_exisits`, "conntection" (several places), `now()` context "Error getting" (`db.rs:17`).
- `tmux_command_without_output`'s error message has an unbalanced backtick and pretty-prints the args across several lines (`utils/tmux.rs:14`).

### L8: Duplicated or wasted work
- `get_directories` and `get_repos` are identical apart from the `.git` check (`commands/open.rs:102-150`).
- In `all` mode, the configured paths are read and canonicalized three times per `open`.
- The score SQL `CASE` is written twice, in `list` and in `scores` (`db.rs:111-121`, `:291-300`).
- `existing_session_names()` runs twice per `open` (`commands/open.rs:28`, `:242`).
- `config::default_session()` runs in every sort comparison, reading an environment variable and allocating a string each time (`commands/open.rs:295`, `:298`).
- `get_client_terminal_size` calls tmux directly instead of going through the wrappers in `tmux.rs` (`utils/mod.rs:13-30`).
- `existing_session_names` does `.context(…).unwrap()`, so the context is pointless and an unexpected line panics (`utils/tmux.rs:83-84`).
