//!
//! # Library backend
//!
//! This handles the logic for the cli tool rost_sessionizer.

/// Module that handles the logic
pub mod commands;

/// Module that provides internally used functions
pub mod utils;

// TODO: #1 Make this configurable <2025-07-10>
const DEFAULT_SESSION: &str = "Default";
