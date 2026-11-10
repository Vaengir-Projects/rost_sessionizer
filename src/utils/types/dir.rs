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
    pub(crate) path: Option<PathBuf>,
    pub(crate) rank: Option<f64>,
}

impl From<(SessionName, Option<PathBuf>, Option<f64>)> for Dir {
    fn from((name, path, rank): (SessionName, Option<PathBuf>, Option<f64>)) -> Self {
        Self { name, path, rank }
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
                    if e.get().rank.is_none() && v.rank.is_some() {
                        e.insert(v);
                    }
                }
            }
        }
    }

    fn sort(&self) -> Vec<Dir> {
        let default_session = config::default_session();
        let mut sorted_vec: Vec<Dir> = self.values().cloned().collect();
        sorted_vec.sort_by(|a, b| {
            if a.name == default_session {
                return std::cmp::Ordering::Less;
            }
            if b.name == default_session {
                return std::cmp::Ordering::Greater;
            }

            match (a.rank, b.rank) {
                (None, Some(_)) => std::cmp::Ordering::Less,
                (Some(_), None) => std::cmp::Ordering::Greater,
                (Some(ra), Some(rb)) => rb.total_cmp(&ra),
                (None, None) => match (&a.path, &b.path) {
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    _ => a.name.cmp(&b.name),
                },
            }
        });

        sorted_vec
    }
}
