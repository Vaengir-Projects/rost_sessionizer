# F11: `SESSIONIZER_PATHS` Isn't Validated

## Where:

`src/config.rs:10-15`; `src/commands/open.rs:105`, `:111`, `:130`, `:136`, `:155`

- Variable unset: `panic!("No PATHS configured")`.
- Empty segment: the common pattern `SESSIONIZER_PATHS="$SESSIONIZER_PATHS:$HOME/x"`, with the variable empty
  beforehand, leaves a leading `:`. `canonicalize("")` then fails, and the **whole** `open` aborts, even when you only
  want an existing session.
- A configured directory that doesn't exist: same as above.
- A configured `/`: `file_name().unwrap()` panics.
