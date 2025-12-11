//!
//! # Library backend
//!
//! This handles the logic for the cli tool `rost_sessionizer`.

/// Module that handles the logic
pub mod commands;

/// Module that provides internally used functions
pub mod utils;

// TODO: #1 Make this configurable <2025-07-10>
const DEFAULT_SESSION: &str = "Default";

// TODO: #1 Make configurable <2025-07-08>
const PATHS: &[&str] = &[
    "/home/vaengir/personal/Bachelor_Latex/",
    "/home/vaengir/vaengir/AwesomeWM/",
    "/home/vaengir/vaengir/CLearn/",
    "/home/vaengir/vaengir/Neovim/",
    "/home/vaengir/vaengir/Scripts/",
    "/home/vaengir/vaengir/ZigLearn/",
    "/home/vaengir/vaengir/dotfiles/",
    "/home/vaengir/vaengir/harpoon/",
    "/home/vaengir/vaengir/quicker.nvim//",
    "/home/vaengir/vaengir/rost_interpreter/",
    "/home/vaengir/vaengir/rost_sessionizer/",
    "/home/vaengir/vaengir/symbols-outline.nvim//",
    "/home/vaengir/vaengir/zig_compiler/",
    "/home/vaengir/vaengir/rigit/",
];
