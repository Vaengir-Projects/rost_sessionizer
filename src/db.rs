use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
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

    Ok(rows.collect::<rusqlite::Result<HashMap<_, _>>>()?)
}
