//!
//! # Session name
//!
//! Provides a tmux session name that is safe to use as a tmux target.

use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use std::fmt;

/// A tmux session name that is safe to use as a tmux target.
///
/// tmux uses `.` and `:` as separators in targets (`session:window.pane`), so a
/// session name containing them can't be addressed reliably.
/// See #20260911-210534 and #20261104-192731.
///
/// # Invariant
///
/// A `SessionName` never contains `.` or `:`. The inner `String` is private, so
/// the only way to create one is [`SessionName::new`]. Every other way of
/// creating one (`From`, `FromSql`, `Deserialize`, ...) must go through `new`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct SessionName(String);

impl SessionName {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// Creates a `SessionName` from `raw`, replacing every `.` and `:` with `_`.
    ///
    /// This is lossy: `foo.bar` and `foo_bar` both become `foo_bar`.
    pub(crate) fn new(raw: &str) -> Self {
        Self(
            raw.chars()
                .map(|c| if matches!(c, '.' | ':') { '_' } else { c })
                .collect(),
        )
    }
}

impl fmt::Display for SessionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl ToSql for SessionName {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for SessionName {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_str().map(Self::new)
    }
}
