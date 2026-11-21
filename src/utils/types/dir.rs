//!
//! # Dir & Dirs & HashMapExtend
//!
//! Provides types and traits for the underlying data.

use crate::{config, utils::types::session_name::SessionName};
use std::{
    collections::{HashMap, hash_map::Entry},
    path::PathBuf,
};

#[derive(Debug, Clone)]
pub(crate) struct Dir {
    pub(crate) name: SessionName,
    pub(crate) path: PathBuf,
    pub(crate) rank: Option<f64>,
    pub(crate) has_session: bool,
}

impl Dir {
    pub(crate) fn session(name: SessionName, path: PathBuf) -> Self {
        Self {
            name,
            path,
            rank: None,
            has_session: true,
        }
    }
}

impl From<(SessionName, PathBuf, Option<f64>)> for Dir {
    fn from((name, path, rank): (SessionName, PathBuf, Option<f64>)) -> Self {
        Self {
            name,
            path,
            rank,
            has_session: false,
        }
    }
}

impl PartialEq for Dir {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

pub(crate) trait HashMapExtend {
    fn try_insert_dir(&mut self, dir: Dir);

    fn try_extend(&mut self, iter: Self);

    fn sort(&self) -> Vec<Dir>;
}

pub(crate) type Dirs = HashMap<SessionName, Dir>;

impl HashMapExtend for Dirs {
    fn try_insert_dir(&mut self, dir: Dir) {
        self.entry(dir.name.clone()).or_insert(dir);
    }

    fn try_extend(&mut self, iter: Self) {
        for (k, v) in iter {
            match self.entry(k) {
                Entry::Vacant(e) => {
                    e.insert(v);
                }
                Entry::Occupied(mut e) => {
                    let existing = e.get_mut();
                    if existing.rank.is_none() && v.rank.is_some() {
                        existing.path = v.path;
                        existing.rank = v.rank;
                    }
                    existing.has_session |= v.has_session;
                }
            }
        }
    }

    fn sort(&self) -> Vec<Dir> {
        let default_session = config::default_session();
        // Entries without a rank sort after every ranked entry.
        let rank = |dir: &Dir| dir.rank.unwrap_or(f64::NEG_INFINITY);

        let mut sorted_vec: Vec<Dir> = self.values().cloned().collect();
        sorted_vec.sort_by(|a, b| {
            // `false` sorts before `true`, so `b` is compared to `a` to put `true` first.
            (b.name == default_session)
                .cmp(&(a.name == default_session))
                .then_with(|| b.has_session.cmp(&a.has_session))
                .then_with(|| rank(b).total_cmp(&rank(a)))
                .then_with(|| a.name.cmp(&b.name))
        });

        sorted_vec
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::HashSet;
    use test_strategy::proptest;

    fn dir() -> impl Strategy<Value = Dir> {
        (
            "[a-d]{1,2}",
            "/[a-z]{1,3}(/[a-z]{1,3}){0,2}",
            prop::option::of(0.0..100.0f64),
            any::<bool>(),
        )
            .prop_map(|(name, path, rank, has_session)| Dir {
                name: SessionName::new(&name),
                path: PathBuf::from(path),
                rank,
                has_session,
            })
    }

    fn insert_all(dirs: &[Dir]) -> Dirs {
        let mut map = Dirs::new();
        for dir in dirs {
            map.try_insert_dir(dir.clone());
        }

        map
    }

    fn all_fields(dir: &Dir) -> (&SessionName, &PathBuf, Option<f64>, bool) {
        (&dir.name, &dir.path, dir.rank, dir.has_session)
    }

    fn named(name: &str, rank: Option<f64>, has_session: bool) -> Dir {
        Dir {
            name: SessionName::new(name),
            path: PathBuf::from(format!("/{name}")),
            rank,
            has_session,
        }
    }

    #[test]
    fn session_entries_are_running_and_unranked() {
        let dir = Dir::session(SessionName::new("api"), PathBuf::from("/api"));
        assert!(dir.has_session);
        assert_eq!(dir.rank, None);
    }

    #[test]
    fn converted_entries_are_not_sessions() {
        let dir = Dir::from((SessionName::new("api"), PathBuf::from("/api"), Some(2.0)));
        assert!(!dir.has_session);
        assert_eq!(dir.rank, Some(2.0));
    }

    #[test]
    fn equality_compares_names_only() {
        assert_eq!(named("api", Some(1.0), true), named("api", None, false));
        assert_ne!(named("api", None, false), named("web", None, false));
    }

    #[test]
    fn ranked_entry_wins_regardless_of_merge_order() {
        let stored = Dir::from((SessionName::new("api"), PathBuf::from("/stored"), Some(3.0)));
        let found = Dir::from((SessionName::new("api"), PathBuf::from("/found"), None));

        for (first, second) in [(&stored, &found), (&found, &stored)] {
            let mut map = insert_all(std::slice::from_ref(first));
            map.try_extend(insert_all(std::slice::from_ref(second)));
            let api = &map[&SessionName::new("api")];
            assert_eq!(api.path, PathBuf::from("/stored"));
            assert_eq!(api.rank, Some(3.0));
        }
    }

    #[test]
    fn sorts_like_the_documented_example() {
        let default = config::default_session();
        let entries = [
            Dir::session(default.clone(), PathBuf::from("/home")),
            named("api", Some(12.0), true),
            named("notes", None, true),
            named("dotfiles", Some(40.0), false),
            named("blog", Some(5.0), false),
            named("alpha", None, false),
            named("zeta", None, false),
        ];
        let sorted: Vec<String> = insert_all(&entries)
            .sort()
            .iter()
            .map(|d| d.name.to_string())
            .collect();
        assert_eq!(
            sorted,
            [
                default.as_str(),
                "api",
                "notes",
                "dotfiles",
                "blog",
                "alpha",
                "zeta"
            ]
        );
    }

    #[proptest]
    fn try_insert_dir_keeps_the_first_entry_per_name(
        #[strategy(prop::collection::vec(dir(), 0..20))] dirs: Vec<Dir>,
    ) {
        let map = insert_all(&dirs);
        let names: HashSet<_> = dirs.iter().map(|d| &d.name).collect();
        prop_assert_eq!(map.len(), names.len());
        for (key, entry) in &map {
            prop_assert_eq!(key, &entry.name);
            let first = dirs.iter().find(|d| &d.name == key).unwrap();
            prop_assert_eq!(all_fields(entry), all_fields(first));
        }
    }

    #[proptest]
    fn try_extend_merges_by_name(
        #[strategy(prop::collection::vec(dir(), 0..20))] existing: Vec<Dir>,
        #[strategy(prop::collection::vec(dir(), 0..20))] incoming: Vec<Dir>,
    ) {
        let before = insert_all(&existing);
        let other = insert_all(&incoming);
        let mut merged = before.clone();
        merged.try_extend(other.clone());

        let all_names: HashSet<_> = before.keys().chain(other.keys()).collect();
        prop_assert_eq!(merged.len(), all_names.len());
        for (name, entry) in &merged {
            match (before.get(name), other.get(name)) {
                (Some(old), None) => prop_assert_eq!(all_fields(entry), all_fields(old)),
                (None, Some(new)) => prop_assert_eq!(all_fields(entry), all_fields(new)),
                (Some(old), Some(new)) => {
                    let takes_rank = old.rank.is_none() && new.rank.is_some();
                    let source = if takes_rank { new } else { old };
                    prop_assert_eq!(&entry.path, &source.path);
                    prop_assert_eq!(entry.rank, source.rank);
                    prop_assert_eq!(entry.has_session, old.has_session || new.has_session);
                }
                (None, None) => prop_assert!(false, "{} came from nowhere", name),
            }
        }
    }

    #[proptest]
    fn sort_returns_every_entry_exactly_once(
        #[strategy(prop::collection::vec(dir(), 0..20))] dirs: Vec<Dir>,
    ) {
        let map = insert_all(&dirs);
        let mut sorted: Vec<_> = map.sort().into_iter().map(|d| d.name).collect();
        let mut expected: Vec<_> = map.into_keys().collect();
        sorted.sort();
        expected.sort();
        prop_assert_eq!(sorted, expected);
    }

    #[proptest]
    fn sort_follows_the_priority_rules(
        #[strategy(prop::collection::vec(dir(), 0..20))] dirs: Vec<Dir>,
        #[any] with_default: bool,
    ) {
        let default = config::default_session();
        let mut map = insert_all(&dirs);
        if with_default {
            map.try_insert_dir(Dir::from((default.clone(), PathBuf::from("/home"), None)));
        }
        let sorted = map.sort();

        if map.contains_key(&default) {
            prop_assert_eq!(&sorted[0].name, &default);
        }
        for pair in sorted.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            prop_assert_ne!(&b.name, &default);
            if a.name == default {
                continue;
            }
            prop_assert!(a.has_session >= b.has_session);
            if a.has_session != b.has_session {
                continue;
            }
            match (a.rank, b.rank) {
                (None, Some(_)) => {
                    prop_assert!(false, "unranked {} before ranked {}", a.name, b.name);
                }
                (Some(x), Some(y)) if x.total_cmp(&y).is_ne() => prop_assert!(x > y),
                (Some(_), None) => {}
                _ => prop_assert!(a.name < b.name),
            }
        }
    }

    #[proptest]
    fn sort_does_not_depend_on_hash_map_order(
        #[strategy(prop::collection::vec(dir(), 0..20))] dirs: Vec<Dir>,
    ) {
        let sorted_names = || {
            let sorted = insert_all(&dirs).sort();
            sorted.into_iter().map(|d| d.name).collect::<Vec<_>>()
        };
        prop_assert_eq!(sorted_names(), sorted_names());
    }
}
