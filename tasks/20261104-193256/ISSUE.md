# F2: Tmux Exit Codes Are Never Checked

## Where:

`src/utils/tmux.rs:10-17`

`Command::status()` only returns an error when the tmux binary can't be started, and a non-zero exit code is discarded.
Every caller of `tmux_command_without_output` (`switch-client`, `kill-session`, `new-session`, `new-window`,
`display-message`, `attach-session`) reports success even when tmux failed. This hides several of the other findings.

## Failure:

`kill` switches to the default session. If that session doesn't exist, the switch fails silently, the current session
is killed anyway, and the client drops out of tmux. `open` reports success even when nothing happened.

# Fix

`tmux_command_without_output` now turns a non-zero exit status into an error that names the command and its status.
`kill` therefore stops before killing the current session when switching to the default session fails, and `open`
reports failed tmux commands instead of exiting successfully.

- It keeps using `status()` instead of `output()`, because `attach-session` needs the terminal. Tmux's own error
  message still reaches stderr.
- `has-session` only answers yes or no, so its exit status stays the answer and its stderr is silenced.
- `tmux ls` exits with 1 when no server is running, so its exit status is still ignored on purpose.
- `get_client_terminal_size` now checks the exit status and includes tmux's stderr in the error.
- `kill-all` tries to kill every session and reports all failures at the end.
- Errors that were hidden before now show up, e.g. F8 (`base-index 0`). When `open` runs via `neww`, use
  `rost_sessionizer open || { printf 'Press Enter to close'; read -r dummy; }` to read them before the window closes.
