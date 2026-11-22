# Notes

I don't want to have to configure every single path to a repo in order for it to show up in rost_sessionizer.
Providing a folder should look through all the subfolders to find possible repos (maybe only for worktree search_mode).
This should also handle the case where a configured path isn't available. The program shouldn't just fail.
Maybe move configuration into `.config/rost_sessionizer/config.toml`?

# Changes

- Configured folders are searched below, down to `depth` levels (default 1). The search doesn't go into repos and skips
  hidden folders, unless a hidden folder is configured itself. `-s dirs` lists every folder found, `-s repos` every
  repo and `-s worktrees` the worktrees of every repo.
- Entries are named after their folder. Names that occur more than once get their parent folder, worktrees are always
  named `<repo>/<worktree>`.
- Settings can be stored in `config.toml` in the config directory: `default_session`, `paths` and `depth`.
  `DEFAULT_SESSION`, `SESSIONIZER_PATHS` and the new `SESSIONIZER_DEPTH` take precedence. `~` gets expanded.
- `rost_sessionizer config example` prints an example config.
- Unavailable paths are handled by #20261104-195203.
