# F4: The Frecency Sort Is Inverted

## Where:

`src/utils/types/dir.rs` (`sort`)

`(None, Some(_)) => Less` puts every entry without a rank before every ranked entry. fzf runs with `--tiebreak=index`,
so with an empty query the list order is the input order: `Default`, open sessions, every unvisited directory from A to
Z, and only then the frecency-ranked entries. The most-used entries end up last. An open session that has a DB entry is
replaced by the ranked entry in `try_extend`, so it also loses its "sessions first" place.

# Fix

`open` now lists the default session first, then open sessions, then other entries with a frecency score (highest
first), and finally everything without a score. Ties are broken by name.
Entries now know whether they belong to a running session (`Dir::has_session`) instead of `sort` guessing it from a
missing path.
