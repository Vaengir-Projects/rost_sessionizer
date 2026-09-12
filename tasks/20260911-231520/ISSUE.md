When opening a new session Neovim, that should be started in the first window, has a weird status.
See [recording-1789169722.mp4](recording-1789169722.mp4)

Apparently neovim was openend in the tmux default size which then caused it to break when receiving a size update
while it was initializing. Providing the size on session creation prevents this issue.
