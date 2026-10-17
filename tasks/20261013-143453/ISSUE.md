I don't want to have to configure every single path to a repo in order for it to show up in rost_sessionizer.
Providing a folder should look through all the subfolders to find possible repos (maybe only for worktree search_mode).
This should also handle the case where a configured path isn't available. The program shouldn't just fail.
Maybe move configuration into `.config/rost_sessionizer/config.yml`?
