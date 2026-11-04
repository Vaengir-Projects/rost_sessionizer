# F8: Window Indices Assume `base-index 1`

## Where:

`src/utils/tmux.rs:23`, `:117`, `:124`; `src/commands/startup.rs:38`, `:58`, `:102`

The code hard-codes windows `:1` and `:2`. With tmux's default `base-index 0`:
- A new session gets windows 0 (Neovim) and 2 (Bash).
- `switch-client -t name:1` fails with "can't find window: 1", and the failure is ignored (F2), so `open` never moves
  the client.
- `startup` outside tmux runs `attach-session -t Default:1`, which fails, so the terminal is never attached.
