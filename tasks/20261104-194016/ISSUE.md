# F5: Entries Without a Path Panic or Fail

## Where:

`src/utils/tmux.rs:92`; `src/commands/open.rs:96`; `src/db.rs:257-259`

- **a)** `create_tmux_session` calls `unwrap()` on `selected_session.path`. Entries for `Default` and for open sessions
  have no path, and `open` always lists `Default` first (`open.rs:32-34`), even when it isn't running. Selecting it
  when it doesn't exist (no `startup` run, or killed by hand) causes a **panic**. The same happens if a listed session
  closes while fzf is open.
- **b)** `db::bump` requires a path. `try_extend` never gives a session entry the path of the matching directory,
  because it only compares ranks. So `open -s dirs` followed by picking an open session, or picking a session made by
  hand (`tmux new -s scratch`), works for the switch. The command then fails with "Error because selection doesn't have
  a path" and exit code 1, and `db::age` is skipped.
