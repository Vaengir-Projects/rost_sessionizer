# F12: The Frecency DB Keeps Stale Paths and Deletes Bookmarks

## Where:

`src/db.rs:251-254`, `:282`

- **a)** `bump`'s `ON CONFLICT(name) DO UPDATE` never updates `path`, and `try_extend` prefers the ranked DB entry over
  the entry found on disk. If a repo moves from `~/code/api` to `~/work/api`, the old DB path wins. Sessions are
  created with the old directory, and `bump` keeps that path until `frecency rm api`.
- **b)** `age()` runs `DELETE … WHERE rank < 1`. Entries created with `frecency new` or `frecency add` start at rank 0.
  The first time the total rank passes 10,000, every bookmark that was never opened is deleted without notice.

# Fix

- **a)** Entries whose path no longer exists are removed when the database is opened, so a moved directory has to be
  added again. `frecency new` with a name that already exists now fails with a clear message instead.
- **b)** `age()` only deletes entries with a rank between 0 and 1. Bookmarks start at 0 and keep it until they're
  opened and an aged entry never reaches 0.
