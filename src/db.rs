use anyhow::{Context, Result, bail};
use rusqlite::{Connection, ErrorCode, params};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    commands::open::{Dir, Dirs},
    config,
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
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS visits (
            name          TEXT    PRIMARY KEY,
            path          TEXT    NOT NULL,
            rank          REAL    NOT NULL DEFAULT 0,
            last_accessed INTEGER NOT NULL
        ) STRICT;",
    )
    .context("Error creating 'visits' table")?;

    Ok(conn)
}

pub(crate) fn new(conn: &Connection, dir: &Dir) -> Result<()> {
    conn.execute(
        "INSERT INTO visits (name, path, rank, last_accessed) VALUES (?1, ?2, 0, ?3)",
        params![
            &dir.name,
            &dir.path
                .clone()
                .context("Error because selection doesn't have a path")?
                .to_str(),
            now()?
        ],
    )
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
    let rows = list(conn).context("Error getting rows from frecency database")?;
    if rows.is_empty() {
        println!("The frecency database is empty.");

        return Ok(());
    }

    let t = now()?;
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
            ago(t - row.last_accessed),
            tilde(&row.path, home.as_deref()),
        );
    }

    Ok(())
}

fn list(conn: &Connection) -> Result<Vec<Row>> {
    let t = now()?;
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
    let rows = stmt.query_map([t], |r| {
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
        match secs.abs() {
            s if s >= 86_400 => Time::Second(s).to_day(),
            s if s >= 3600 => Time::Second(s).to_hour(),
            s if s >= 60 => Time::Second(s).to_minute(),
            _ => Time::Second(secs).to_second(),
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

pub(crate) fn modify(conn: &Connection, old_name: &str, new_name: &str) -> Result<()> {
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

pub(crate) fn remove(conn: &Connection, name: &str) -> Result<()> {
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
            params![
                &dir.name,
                &dir.path
                    .clone()
                    .context("Error because selection doesn't have a path")?
                    .to_str(),
                now()?
            ],
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
        tx.execute("DELETE FROM visits WHERE rank < 1", [])?;
    }

    tx.commit().context("Error commit the transaction")
}

/// Returns path -> Dir (with frecency score) for all known folders.
pub(crate) fn scores(conn: &Connection) -> Result<Dirs> {
    let t = now()?;
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

    let rows = stmt.query_map([t], |r| {
        let name: String = r.get(0)?;
        let path = PathBuf::from(r.get::<_, String>(1)?);
        let score: f64 = r.get(2)?;

        Ok((name.clone(), Dir::from((name, Some(path), Some(score)))))
    })?;

    Ok(rows.collect::<rusqlite::Result<_>>()?)
}
