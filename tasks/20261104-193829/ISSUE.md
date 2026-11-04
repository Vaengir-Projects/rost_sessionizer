# F4: The Frecency Sort Is Inverted

## Where:

`src/commands/open.rs:302-305`

`(None, Some(_)) => Less` puts every entry without a rank before every ranked entry. fzf runs with `--tiebreak=index`,
so with an empty query the list order is the input order: `Default`, open sessions, every unvisited directory from A to
Z, and only then the frecency-ranked entries. The most-used entries end up last. An open session that has a DB entry is
replaced by the ranked entry in `try_extend`, so it also loses its "sessions first" place.
