# F7: Session Targets Match by Prefix

## Where:

`src/utils/tmux.rs:23`, `:31`, `:43`, `:117`; `src/commands/startup.rs:58`, `:102`

tmux resolves `-t name` by exact match first, then by prefix, then as an fnmatch pattern.

## Failure:

Only `repo/main` is open and you pick `repo`. `has-session -t repo` matches `repo/main` and returns true, so the tool
switches into `repo/main` and the `repo` session is never created.

# Fix

Every `-t` target now uses `SessionName::target()`, which prefixes the name with `=`. tmux then only accepts an exact
match, so `repo` no longer resolves to `repo/main`.
