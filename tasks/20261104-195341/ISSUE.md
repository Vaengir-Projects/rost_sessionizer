# F12: The Frecency DB Keeps Stale Paths and Deletes Bookmarks

## Where:

`src/db.rs:251-254`, `:282`

- **a)** `bump`'s `ON CONFLICT(name) DO UPDATE` never updates `path`, and `try_extend` prefers the ranked DB entry over
  the entry found on disk. If a repo moves from `~/code/api` to `~/work/api`, the old DB path wins. Sessions are
  created with the old directory, and `bump` keeps that path until `frecency rm api`.
- **b)** `age()` runs `DELETE … WHERE rank < 1`. Entries created with `frecency new` or `frecency add` start at rank 0.
  The first time the total rank passes 10,000, every bookmark that was never opened is deleted without notice.
