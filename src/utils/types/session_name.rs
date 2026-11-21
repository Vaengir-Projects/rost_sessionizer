//!
//! # Session name
//!
//! Provides a tmux session name that is safe to use as a tmux target.

use anyhow::{Result, bail};
use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use std::{fmt, str::FromStr};

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

    /// A tmux target that only matches this exact session, never a prefix or pattern.
    pub(crate) fn target(&self) -> String {
        format!("={}", self.0)
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

impl FromStr for SessionName {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> Result<Self> {
        if name.trim().is_empty() {
            bail!("Error: name must contain at least one non-whitespace character");
        }
        // This is a limitation by tmux. See #20260911-210534
        if name.contains(['.', ':']) {
            bail!("Error: name must not contain '.' or ':', use '_' or '-' instead");
        }

        Ok(Self::new(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rusqlite::Connection;
    use test_strategy::proptest;

    fn raw_name() -> impl Strategy<Value = String> {
        prop_oneof!["[a-zA-Z0-9._:/ -]{0,40}", any::<String>()]
    }

    #[test]
    fn replaces_dots_and_colons() {
        assert_eq!(
            SessionName::new("symbols-outline.nvim").as_str(),
            "symbols-outline_nvim"
        );
        assert_eq!(SessionName::new("a:b.c").as_str(), "a_b_c");
    }

    #[test]
    fn keeps_slashes() {
        assert_eq!(SessionName::new("repo/main").as_str(), "repo/main");
    }

    #[test]
    fn target_matches_exactly() {
        assert_eq!(SessionName::new("repo").target(), "=repo");
    }

    #[test]
    fn parse_rejects_empty_name() {
        assert!("".parse::<SessionName>().is_err());
    }

    #[proptest]
    fn never_contains_separators(#[strategy(raw_name())] raw: String) {
        prop_assert!(!SessionName::new(&raw).as_str().contains(['.', ':']));
    }

    #[proptest]
    fn changes_only_separators(#[strategy(raw_name())] raw: String) {
        let name = SessionName::new(&raw);
        prop_assert_eq!(name.as_str().chars().count(), raw.chars().count());
        for (before, after) in raw.chars().zip(name.as_str().chars()) {
            let expected = if matches!(before, '.' | ':') {
                '_'
            } else {
                before
            };
            prop_assert_eq!(after, expected);
        }
    }

    #[proptest]
    fn is_idempotent(#[strategy(raw_name())] raw: String) {
        let once = SessionName::new(&raw);
        prop_assert_eq!(SessionName::new(once.as_str()), once);
    }

    #[proptest]
    fn target_is_the_name_with_equals_sign(#[strategy(raw_name())] raw: String) {
        let name = SessionName::new(&raw);
        prop_assert_eq!(name.target(), format!("={name}"));
    }

    #[proptest]
    fn display_matches_as_str(#[strategy(raw_name())] raw: String) {
        let name = SessionName::new(&raw);
        prop_assert_eq!(name.to_string(), name.as_str());
    }

    #[proptest]
    fn survives_a_database_round_trip(#[strategy(raw_name())] raw: String) {
        let conn = Connection::open_in_memory().unwrap();
        let name = SessionName::new(&raw);
        let back: SessionName = conn.query_row("SELECT ?1", [&name], |r| r.get(0)).unwrap();
        prop_assert_eq!(back, name);
    }

    #[proptest]
    fn reading_from_the_database_sanitizes(#[strategy(raw_name())] raw: String) {
        let conn = Connection::open_in_memory().unwrap();
        let back: SessionName = conn.query_row("SELECT ?1", [&raw], |r| r.get(0)).unwrap();
        prop_assert_eq!(back, SessionName::new(&raw));
    }

    #[proptest]
    fn parse_rejects_whitespace_only(#[strategy("\\s{1,10}")] name: String) {
        prop_assert!(name.parse::<SessionName>().is_err());
    }

    #[proptest]
    fn parse_rejects_separators(#[strategy("[^.:]{0,10}[.:][^.:]{0,10}")] name: String) {
        prop_assert!(name.parse::<SessionName>().is_err());
    }

    #[proptest]
    fn parse_accepts_everything_else_unchanged(
        #[strategy("[^.:]{0,10}[^.:\\s][^.:]{0,10}")] name: String,
    ) {
        let parsed = name.parse::<SessionName>().unwrap();
        prop_assert_eq!(parsed.as_str(), name);
    }
}
