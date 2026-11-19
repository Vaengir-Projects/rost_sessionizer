use crate::{
    config,
    utils::types::{
        dir::{Dir, Dirs},
        session_name::SessionName,
    },
};
use anyhow::{Context, Result, anyhow, bail};
use rusqlite::{Connection, ErrorCode, params};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn now() -> Result<i64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("Error getting")?
        .as_secs()
        .cast_signed())
}

fn db_path() -> Result<PathBuf> {
    let base = dirs::data_dir()
        .context("Data directory not set")?
        .join("rost_sessionizer");
    fs::create_dir_all(&base).context("Couldn't create data directory")?;

    Ok(base.join("frecency.db"))
}

pub(crate) fn open() -> Result<Connection> {
    let path = db_path()?;
    let conn = Connection::open(&path)
        .with_context(|| format!("Error opening database at {}", path.display()))?;

    conn.pragma_update(None, "journal_mode", "WAL")
        .context("Error setting pragma update")?;
    conn.busy_timeout(std::time::Duration::from_millis(500))
        .context("Error setting busy timeout")?;
    init(&conn)?;

    Ok(conn)
}

fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS visits (
            name          TEXT    PRIMARY KEY,
            path          TEXT    NOT NULL,
            rank          REAL    NOT NULL DEFAULT 0,
            last_accessed INTEGER NOT NULL
        ) STRICT;",
    )
    .context("Error creating 'visits' table")?;
    remove_stale(conn).context("Error removing entries whose path no longer exists")
}

fn remove_stale(conn: &Connection) -> Result<()> {
    let rows: Vec<(String, String)> = conn
        .prepare("SELECT name, path FROM visits")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    for (name, path) in rows {
        if matches!(Path::new(&path).try_exists(), Ok(false)) {
            conn.execute("DELETE FROM visits WHERE name = ?1", [name])?;
        }
    }

    Ok(())
}

pub(crate) fn new(conn: &Connection, dir: &Dir) -> Result<()> {
    conn.execute(
        "INSERT INTO visits (name, path, rank, last_accessed) VALUES (?1, ?2, 0, ?3)",
        params![&dir.name, &dir.path.to_str(), now()?],
    )
    .map_err(|e| match e {
        rusqlite::Error::SqliteFailure(ref err, _)
            if err.code == ErrorCode::ConstraintViolation =>
        {
            anyhow!("Error: An entry named '{}' already exists", dir.name)
        }
        other => anyhow::Error::from(other),
    })
    .context("Error executing sql statement to insert entry into frecency database")?;

    Ok(())
}

struct Row {
    name: String,
    path: String,
    rank: f64,
    last_accessed: i64,
    score: f64,
}

pub(crate) fn select_all(conn: &Connection) -> Result<()> {
    let t = now()?;
    let rows = list(conn, t).context("Error getting rows from frecency database")?;
    if rows.is_empty() {
        println!("The frecency database is empty.");

        return Ok(());
    }

    let home = dirs::home_dir();
    let name_size = biggest_name(&rows)?;
    println!(
        "{:<name_size$} | {:>7} | {:>6} | {:<10} | PATH",
        "NAME", "SCORE", "RANK", "LAST USED"
    );
    for row in &rows {
        println!(
            "{:<name_size$} | {:>7.1} | {:>6.1} | {:<10} | {}",
            row.name,
            row.score,
            row.rank,
            ago(t.saturating_sub(row.last_accessed)),
            tilde(&row.path, home.as_deref()),
        );
    }

    Ok(())
}

fn list(conn: &Connection, now: i64) -> Result<Vec<Row>> {
    let mut stmt = conn.prepare(
        "SELECT name, path, rank, last_accessed,
                rank * CASE
                    WHEN ?1 - last_accessed < 3600   THEN 4.0
                    WHEN ?1 - last_accessed < 86400  THEN 2.0
                    WHEN ?1 - last_accessed < 604800 THEN 0.5
                    ELSE 0.25
                END AS score
         FROM visits
         ORDER BY score DESC, path ASC",
    )?;
    let rows = stmt.query_map([now], |r| {
        Ok(Row {
            name: r.get(0)?,
            path: r.get(1)?,
            rank: r.get(2)?,
            last_accessed: r.get(3)?,
            score: r.get(4)?,
        })
    })?;

    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn biggest_name(rows: &[Row]) -> Result<usize> {
    Ok(rows
        .iter()
        .map(|r| r.name.len())
        .max()
        .context("Error determining the biggest name length")?
        + 3)
}

#[derive(Debug)]
enum Time {
    Second(i64),
    Minute(i64),
    Hour(i64),
    Day(i64),
}

impl Time {
    fn to_second(&self) -> Self {
        match self {
            Time::Second(i) => Time::Second(*i),
            Time::Minute(i) => Time::Second(i * 60),
            Time::Hour(i) => Time::Second(i * 3600),
            Time::Day(i) => Time::Second(i * 86_400),
        }
    }

    fn to_minute(&self) -> Self {
        match self {
            Time::Second(i) => Time::Minute(i / 60),
            Time::Minute(i) => Time::Minute(*i),
            Time::Hour(i) => Time::Minute(i * 60),
            Time::Day(i) => Time::Minute(i * 1440),
        }
    }

    fn to_hour(&self) -> Self {
        match self {
            Time::Second(i) => Time::Hour(i / 3600),
            Time::Minute(i) => Time::Hour(i / 60),
            Time::Hour(i) => Time::Hour(*i),
            Time::Day(i) => Time::Hour(i * 24),
        }
    }

    fn to_day(&self) -> Self {
        match self {
            Time::Second(i) => Time::Day(i / 86_400),
            Time::Minute(i) => Time::Day(i / 1440),
            Time::Hour(i) => Time::Day(i / 24),
            Time::Day(i) => Time::Day(*i),
        }
    }
}

impl From<i64> for Time {
    fn from(secs: i64) -> Self {
        match secs {
            s if s >= 86_400 => Self::Second(s).to_day(),
            s if s >= 3600 => Self::Second(s).to_hour(),
            s if s >= 60 => Self::Second(s).to_minute(),
            _ => Self::Second(secs).to_second(),
        }
    }
}

fn ago(secs: i64) -> String {
    match Time::from(secs) {
        Time::Second(_) => "just now".into(),
        Time::Minute(m) => format!("{m}m ago"),
        Time::Hour(h) => format!("{h}h ago"),
        Time::Day(d) => format!("{d}d ago"),
    }
}

fn tilde(path: &str, home: Option<&Path>) -> String {
    match home.and_then(|h| Path::new(path).strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.to_owned(),
    }
}

pub(crate) fn modify(
    conn: &Connection,
    old_name: &SessionName,
    new_name: &SessionName,
) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE visits SET name = ?1 WHERE name = ?2",
            [new_name, old_name],
        )
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(ref err, _)
                if err.code == ErrorCode::ConstraintViolation =>
            {
                anyhow::anyhow!("Error: An entry named '{new_name}' already exists")
            }
            other => anyhow::Error::from(other),
        })
        .with_context(|| format!("Error: Renaming '{old_name}' to '{new_name}'"))?;

    if changed == 0 {
        bail!("Error: No entry named '{old_name}' found");
    }
    Ok(())
}

pub(crate) fn remove(conn: &Connection, name: &SessionName) -> Result<()> {
    conn.execute("DELETE FROM visits WHERE name = ?1", params![name])
        .context("Error executing sql statement to remove entry from the frecency database")?;

    Ok(())
}

/// Called when user selects a folder.
pub(crate) fn bump(conn: &Connection, dir: &Dir) -> Result<()> {
    if dir.name != config::default_session() {
        conn.execute(
            "INSERT INTO visits (name, path, rank, last_accessed) VALUES (?1, ?2, 1, ?3)
         ON CONFLICT(name) DO UPDATE SET
             rank = rank + 1,
             last_accessed = excluded.last_accessed",
            params![&dir.name, &dir.path.to_str(), now()?],
        )
        .context("Error executing sql statement to bump frecency")?;
    }

    Ok(())
}

/// zoxide-style aging: Keep total rank bounded, drop faded entries.
pub(crate) fn age(conn: &mut Connection) -> Result<()> {
    const MAX_AGE: f64 = 10000.0;
    let tx = conn.transaction().context("Error creating a transaction")?;
    let total: f64 = tx
        .query_row("SELECT COALESCE(SUM(rank), 0) FROM visits", [], |r| {
            r.get(0)
        })
        .context("Error querying for total rank values")?;
    if total > MAX_AGE {
        let factor = 0.9 * MAX_AGE / total;
        tx.execute("UPDATE visits SET rank = rank * ?1", [factor])?;
        tx.execute("DELETE FROM visits WHERE rank > 0 AND rank < 1", [])?;
    }

    tx.commit().context("Error commit the transaction")
}

/// Returns session name -> Dir (with frecency score) for all known folders.
pub(crate) fn scores(conn: &Connection) -> Result<Dirs> {
    scores_at(conn, now()?)
}

fn scores_at(conn: &Connection, now: i64) -> Result<Dirs> {
    let mut stmt = conn.prepare(
        "SELECT name, path,
                rank * CASE
                    WHEN ?1 - last_accessed < 3600   THEN 4.0
                    WHEN ?1 - last_accessed < 86400  THEN 2.0
                    WHEN ?1 - last_accessed < 604800 THEN 0.5
                    ELSE 0.25
                END AS score
         FROM visits",
    )?;

    let rows = stmt.query_map([now], |r| {
        let name: SessionName = r.get(0)?;
        let path = PathBuf::from(r.get::<_, String>(1)?);
        let score: f64 = r.get(2)?;

        Ok((name.clone(), Dir::from((name, path, Some(score)))))
    })?;

    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rusqlite::OptionalExtension;
    use std::collections::BTreeMap;
    use tempfile::TempDir;

    const NOW: i64 = 1_800_000_000;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();

        conn
    }

    fn insert(conn: &Connection, name: &str, path: &str, rank: f64, last_accessed: i64) {
        conn.execute(
            "INSERT INTO visits (name, path, rank, last_accessed) VALUES (?1, ?2, ?3, ?4)",
            params![name, path, rank, last_accessed],
        )
        .unwrap();
    }

    fn row(conn: &Connection, name: &str) -> Option<(String, f64, i64)> {
        conn.query_row(
            "SELECT path, rank, last_accessed FROM visits WHERE name = ?1",
            [name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .unwrap()
    }

    fn all_ranks(conn: &Connection) -> BTreeMap<String, f64> {
        let mut stmt = conn.prepare("SELECT name, rank FROM visits").unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    fn entry(name: &str, path: &str) -> Dir {
        Dir::from((SessionName::new(name), PathBuf::from(path), None))
    }

    fn check_aging(before: &[f64], after: &BTreeMap<String, f64>) -> Result<(), TestCaseError> {
        let total: f64 = before.iter().sum();
        let factor = if total > 10_000.0 {
            0.9 * 10_000.0 / total
        } else {
            1.0
        };
        for (i, rank) in before.iter().enumerate() {
            let name = i.to_string();
            let expected = rank * factor;
            if *rank == 0.0 {
                prop_assert_eq!(after.get(&name).copied(), Some(0.0));
            } else if expected < 1.0 {
                prop_assert!(!after.contains_key(&name));
            } else {
                let actual = after.get(&name).copied().unwrap_or(f64::NAN);
                prop_assert!((actual - expected).abs() <= 1e-9 * expected);
            }
        }
        let bound = if total > 10_000.0 { 9_000.0 } else { total };
        prop_assert!(after.values().sum::<f64>() <= bound + 1e-6);

        Ok(())
    }

    mod schema_and_stale_entries {
        use super::*;

        #[test]
        fn init_is_idempotent() {
            let conn = memory_db();
            insert(&conn, "root", "/", 1.0, NOW);
            init(&conn).unwrap();
            assert!(row(&conn, "root").is_some());
        }

        #[test]
        fn init_removes_stale_entries() {
            let tmp = TempDir::new().unwrap();
            let kept = tmp.path().join("kept");
            let gone = tmp.path().join("gone");
            fs::create_dir(&kept).unwrap();
            fs::create_dir(&gone).unwrap();

            let conn = memory_db();
            insert(&conn, "kept", kept.to_str().unwrap(), 1.0, NOW);
            insert(&conn, "gone", gone.to_str().unwrap(), 1.0, NOW);
            fs::remove_dir(&gone).unwrap();
            init(&conn).unwrap();

            assert!(row(&conn, "kept").is_some());
            assert!(row(&conn, "gone").is_none());
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(32))]

            #[test]
            fn init_keeps_existing_paths(
                entries in prop::collection::btree_map("[a-z]{1,8}", any::<bool>(), 0..10),
            ) {
                let tmp = TempDir::new().unwrap();
                let conn = memory_db();
                for (name, exists) in &entries {
                    let path = tmp.path().join(name);
                    if *exists {
                        fs::create_dir(&path).unwrap();
                    }
                    insert(&conn, name, path.to_str().unwrap(), 1.0, NOW);
                }
                init(&conn).unwrap();

                let remaining: Vec<_> = all_ranks(&conn).into_keys().collect();
                let existing: Vec<_> = entries
                    .iter()
                    .filter(|(_, e)| **e)
                    .map(|(n, _)| n.clone())
                    .collect();
                prop_assert_eq!(remaining, existing);
            }
        }
    }

    mod new {
        use super::*;

        #[test]
        fn adds_bookmark_with_rank_zero() {
            let conn = memory_db();
            let before = now().unwrap();
            new(&conn, &entry("api", "/work/api")).unwrap();
            let after = now().unwrap();

            let (path, rank, last_accessed) = row(&conn, "api").unwrap();
            assert_eq!(path, "/work/api");
            assert_eq!(rank, 0.0);
            assert!((before..=after).contains(&last_accessed));
        }

        #[test]
        fn rejects_existing_name() {
            let conn = memory_db();
            new(&conn, &entry("api", "/work/api")).unwrap();
            let error = new(&conn, &entry("api", "/personal/api")).unwrap_err();

            assert!(format!("{error:#}").contains("already exists"));
            assert_eq!(row(&conn, "api").unwrap().0, "/work/api");
        }
    }

    mod bump {
        use super::*;

        #[test]
        fn adds_unknown_entries_with_rank_one() {
            let conn = memory_db();
            bump(&conn, &entry("api", "/work/api")).unwrap();

            let (path, rank, _) = row(&conn, "api").unwrap();
            assert_eq!(path, "/work/api");
            assert_eq!(rank, 1.0);
        }

        #[test]
        fn refreshes_last_accessed_but_keeps_the_path() {
            let conn = memory_db();
            insert(&conn, "api", "/stored", 2.0, 0);
            let before = now().unwrap();
            bump(&conn, &entry("api", "/elsewhere")).unwrap();

            let (path, rank, last_accessed) = row(&conn, "api").unwrap();
            assert_eq!(path, "/stored");
            assert_eq!(rank, 3.0);
            assert!(last_accessed >= before);
        }

        #[test]
        fn ignores_default_session() {
            let conn = memory_db();
            let default = config::default_session();
            bump(
                &conn,
                &Dir::from((default.clone(), PathBuf::from("/home"), None)),
            )
            .unwrap();
            assert!(row(&conn, default.as_str()).is_none());
        }

        proptest! {
            #[test]
            fn counts_every_visit(visits in 1..50u32) {
                let conn = memory_db();
                for _ in 0..visits {
                    bump(&conn, &entry("api", "/work/api")).unwrap();
                }
                let rank = row(&conn, "api").unwrap().1;
                prop_assert!(rank.total_cmp(&f64::from(visits)).is_eq());
            }
        }
    }

    mod modify {
        use super::*;

        #[test]
        fn renames_entry() {
            let conn = memory_db();
            insert(&conn, "api", "/work/api", 2.0, NOW);
            modify(
                &conn,
                &SessionName::new("api"),
                &SessionName::new("work-api"),
            )
            .unwrap();

            assert!(row(&conn, "api").is_none());
            assert_eq!(
                row(&conn, "work-api").unwrap(),
                ("/work/api".to_owned(), 2.0, NOW)
            );
        }

        #[test]
        fn refuses_overwrite_of_entry() {
            let conn = memory_db();
            insert(&conn, "api", "/work/api", 1.0, NOW);
            insert(&conn, "web", "/work/web", 1.0, NOW);
            let error =
                modify(&conn, &SessionName::new("api"), &SessionName::new("web")).unwrap_err();

            assert!(format!("{error:#}").contains("already exists"));
            assert_eq!(row(&conn, "api").unwrap().0, "/work/api");
            assert_eq!(row(&conn, "web").unwrap().0, "/work/web");
        }

        #[test]
        fn reports_missing_entry() {
            let conn = memory_db();
            let error =
                modify(&conn, &SessionName::new("api"), &SessionName::new("web")).unwrap_err();
            assert!(format!("{error:#}").contains("No entry named 'api'"));
        }
    }

    mod remove {
        use super::*;

        #[test]
        fn deletes_entry() {
            let conn = memory_db();
            insert(&conn, "api", "/work/api", 1.0, NOW);
            remove(&conn, &SessionName::new("api")).unwrap();
            assert!(row(&conn, "api").is_none());
        }

        #[test]
        fn missing_entry_is_silent() {
            let conn = memory_db();
            assert!(remove(&conn, &SessionName::new("api")).is_ok());
        }
    }

    mod age {
        use super::*;

        #[test]
        fn scales_ranks_and_keeps_bookmarks() {
            let mut conn = memory_db();
            insert(&conn, "big", "/big", 20_000.0, NOW);
            insert(&conn, "small", "/small", 1.0, NOW);
            insert(&conn, "bookmark", "/bookmark", 0.0, NOW);
            age(&mut conn).unwrap();

            let after = all_ranks(&conn);
            let factor = 0.9 * 10_000.0 / 20_001.0;
            assert!((after["big"] - 20_000.0 * factor).abs() < 1e-9);
            assert!(!after.contains_key("small"));
            assert_eq!(after["bookmark"], 0.0);
        }

        proptest! {
            #[test]
            fn bounds_total_and_keeps_bookmarks(
                ranks in prop::collection::vec(prop_oneof![Just(0.0), 1.0..5_000.0f64], 0..20),
            ) {
                let mut conn = memory_db();
                for (i, rank) in ranks.iter().enumerate() {
                    insert(&conn, &i.to_string(), "/", *rank, NOW);
                }
                age(&mut conn).unwrap();
                check_aging(&ranks, &all_ranks(&conn))?;
            }

            #[test]
            fn aged_ranks_never_reach_zero(
                ranks in prop::collection::vec(1.0..5_000.0f64, 0..20),
                rounds in 1..10usize,
            ) {
                let mut conn = memory_db();
                for (i, rank) in ranks.iter().enumerate() {
                    insert(&conn, &i.to_string(), "/", *rank, NOW);
                }
                for _ in 0..rounds {
                    conn.execute(
                        "INSERT OR REPLACE INTO visits VALUES ('pressure', '/', 20000, ?1)",
                        [NOW],
                    )
                    .unwrap();
                    age(&mut conn).unwrap();
                    prop_assert!(all_ranks(&conn).values().all(|rank| *rank >= 1.0));
                }
            }
        }
    }

    mod scores_and_list {
        use super::*;

        #[test]
        fn scores_weight_rank_by_last_visit() {
            let cases: [(i64, f64); 8] = [
                (0, 4.0),
                (3_599, 4.0),
                (3_600, 2.0),
                (86_399, 2.0),
                (86_400, 0.5),
                (604_799, 0.5),
                (604_800, 0.25),
                (10_000_000, 0.25),
            ];
            let conn = memory_db();
            for (i, (seconds_ago, _)) in cases.iter().enumerate() {
                insert(&conn, &format!("e{i}"), "/", 10.0, NOW - seconds_ago);
            }

            let scores = scores_at(&conn, NOW).unwrap();
            for (i, (seconds_ago, factor)) in cases.iter().enumerate() {
                let score = scores[&SessionName::new(&format!("e{i}"))].rank;
                assert_eq!(score, Some(10.0 * factor), "{seconds_ago} seconds ago");
            }
        }

        proptest! {
            #[test]
            fn scores_and_list_agree(
                entries in prop::collection::btree_map(
                    "[a-z]{1,8}",
                    (prop_oneof![Just(0.0), 0.0..1_000.0f64], 0..2_000_000i64),
                    0..20,
                ),
            ) {
                let conn = memory_db();
                for (name, (rank, ago)) in &entries {
                    insert(&conn, name, "/", *rank, NOW - ago);
                }
                let scores = scores_at(&conn, NOW).unwrap();
                let listed = list(&conn, NOW).unwrap();
                prop_assert_eq!(listed.len(), scores.len());
                for row in &listed {
                    let score = scores[&SessionName::new(&row.name)].rank;
                    prop_assert_eq!(score, Some(row.score));
                }
            }

            #[test]
            fn list_is_sorted_by_score_then_path(
                entries in prop::collection::btree_map(
                    "[a-z]{1,8}",
                    (prop_oneof![Just(0.0), 0.0..1_000.0f64], 0..2_000_000i64, "/[a-z]{1,4}"),
                    0..20,
                ),
            ) {
                let conn = memory_db();
                for (name, (rank, ago, path)) in &entries {
                    insert(&conn, name, path, *rank, NOW - ago);
                }
                let listed = list(&conn, NOW).unwrap();
                for pair in listed.windows(2) {
                    let (a, b) = (&pair[0], &pair[1]);
                    prop_assert!(a.score >= b.score);
                    if a.score.total_cmp(&b.score).is_eq() {
                        prop_assert!(a.path <= b.path);
                    }
                }
            }
        }
    }
}
