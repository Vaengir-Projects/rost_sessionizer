# F9: `kill-all` Spares Sessions by Substring Match

## Where:

`src/commands/kill.rs:43`

`retain(|s| !s.contains(&default))` tests for a substring. With `DEFAULT_SESSION=main`, the sessions `domain-api` and
`main-old` also survive. A short default name like `a` spares almost every session.

# Fix

`kill-all` now runs a single `kill-session -a -t =<default>`, which kills every session except the default one. This
replaces the substring check and the loop from #20261104-193256, which also stopped halfway when it killed the session
`kill-all` itself was running in.
