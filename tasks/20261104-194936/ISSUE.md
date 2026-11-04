# F10: Entries with the Same Name Overwrite Each Other

## Where:

`src/utils/mod.rs:32-37`; `src/commands/open.rs:114`, `:139`, `:176`, `:204`

- **a)** `sanitize_selections` collects into a `HashMap`. When two keys sanitize to the same string (directory
  `foo.bar` and session `foo_bar`), one entry is dropped. Which one survives depends on the HashMap's random iteration
  order, and the rank preference in `try_extend` is skipped.
- **b)** Directories are keyed by basename with `or_insert`. With `SESSIONIZER_PATHS=~/work/api:~/personal/api`, only
  the first `api` shows up and `~/personal/api` can't be opened. The same thing happens between `get_directories`,
  `get_repos` and `get_worktrees` through `try_extend`.
