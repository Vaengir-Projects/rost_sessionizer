# F1: Names Containing `.` or `:` Break Sessions Again (Regression)

## Where:

`src/commands/open.rs:81`, `:264`; `src/utils/mod.rs:32-37`

Commit `fa76fbf` (task #20260911-210534) fixed this by sanitizing the keys of the `Dirs` map. At that time `Dirs` was
`HashMap<String, Option<PathBuf>>`, so the key *was* the name. Since the frecency work (merged in `3e01933`), the
values are `Dir { name, path, rank }` and `select_via_fzf` returns the `Dir`. `sanitize_selections` still only rewrites
the keys, so `dir.name` keeps the raw name.

## Failure: You Pick `symbols-outline.nvim`.

- `has-session -t symbols-outline.nvim` reads `.` as a separator and returns false.
- `new-session -s` creates `symbols-outline_nvim` because tmux sanitizes the name itself.
- `new-window -t symbols-outline.nvim:2` and `switch-client` then fail, and the failure is ignored (F2).
- Every later `open` tries to create the session again.
- `bump` stores the raw name in the DB, and `frecency rm` can't remove it because `validate_name` rejects `.`.
