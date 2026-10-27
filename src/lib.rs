//!
//! # Library backend
//!
//! This handles the logic for the cli tool `rost_sessionizer`.

/// Module that handles the logic.
pub mod commands;

/// Module that handles the configuration.
pub(crate) mod config;

/// Module that provides internally used functions.
pub(crate) mod utils;

/// Module that handles the frecency database.
pub(crate) mod db;
