# F6: Cancelling Fzf Is Reported as an Error

## Where:

`src/commands/open.rs:253-262`

When you press Esc or Ctrl-C, fzf exits with code 130 and empty stdout. The empty string isn't one of the options, so
the tool prints "Error while running the open command: Error selecting new or existing session: Selected value isn't
part of provided options" and exits with code 1.
