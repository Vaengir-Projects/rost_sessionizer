# F10: Entries with the Same Name Overwrite Each Other

## Where:

`src/utils/types/dir.rs` (`try_insert_dir`, `try_extend`); `src/commands/open.rs` (`get_directories`, `get_repos`,
`get_worktrees`)

- **a)** Since #20261104-192731, names are sanitized when the `SessionName` is created, so the map key always equals
  `Dir.name` and merging is deterministic. Entries whose names sanitize to the same `SessionName` (directory `foo.bar`
  and directory `foo_bar`) still collapse into one: `try_insert_dir` keeps the first one inserted, `try_extend` prefers
  a ranked one. Tmux can't hold two sessions with the same name either, so this needs the same disambiguation as b).
- **b)** Directories are keyed by basename with `or_insert`. With `SESSIONIZER_PATHS=~/work/api:~/personal/api` only
  the first `api` shows up and `~/personal/api` can't be opened. The same thing happens between `get_directories`,
  `get_repos` and `get_worktrees` through `try_extend`.
