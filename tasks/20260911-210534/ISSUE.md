Outputs something along the lines of:
```
> ./vaengir/rost_sessionizer/10-configure-worktrees/target/release/rost_sessionizer open --git worktrees
can't find window: symbols-outline
can't find session: symbols-outline.nvim/main
can't find session: symbols-outline.nvim/main
can't find session: symbols-outline.nvim/main
can't find session: symbols-outline.nvim/main
can't find session: symbols-outline.nvim/main
```

Tmux uses '.' and ':' as a separator in its target syntax. The form is as follows: `session:window.pane`.
The solution is to sanitize '.' and ':' to '_' in session names.
