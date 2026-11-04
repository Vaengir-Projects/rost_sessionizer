# F2: Tmux Exit Codes Are Never Checked

## Where:

`src/utils/tmux.rs:10-17`

`Command::status()` only returns an error when the tmux binary can't be started, and a non-zero exit code is discarded.
Every caller of `tmux_command_without_output` (`switch-client`, `kill-session`, `new-session`, `new-window`,
`display-message`, `attach-session`) reports success even when tmux failed. This hides several of the other findings.

## Failure:

`kill` switches to the default session. If that session doesn't exist, the switch fails silently, the current session
is killed anyway, and the client drops out of tmux. `open` reports success even when nothing happened.

