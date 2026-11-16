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
