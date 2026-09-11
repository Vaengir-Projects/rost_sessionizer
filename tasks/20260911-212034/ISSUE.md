Right now all the directories the tool should work for are setup using a Vec<PathBuf>.
This is not ideal especially since editing this Vector would require recompiling the program.
Either a command line argument or a environment variable would be better.

This also applies to the default session name and layout.

This was closed without the layout of the Default Session, or any session for that matter being configurable.
See #20260911-210132 for this.
