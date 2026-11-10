# F3: `kill` Can Hit the Wrong Session

## Where:

`src/utils/tmux.rs` (`current_session`, `existing_sessions`)

`current_session` returns the first line of `tmux ls` that contains "attached". That is the alphabetically first
session with *any* client attached, not the session of the client that ran the command. A session whose name contains
"attached" also matches.

## Failure:

- Terminal A is in `Default` and terminal B is in `proj`. Running `kill` in B gets `Default` back: B is switched to
  `Default` and shown "Can't kill the default session", and `proj` survives.
- With A in `alpha` and B in `zeta`, `kill` in B kills A's session.
