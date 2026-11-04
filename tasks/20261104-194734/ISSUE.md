# F9: `kill-all` Spares Sessions by Substring Match

## Where:

`src/commands/kill.rs:43`

`retain(|s| !s.contains(&default))` tests for a substring. With `DEFAULT_SESSION=main`, the sessions `domain-api` and
`main-old` also survive. A short default name like `a` spares almost every session.
