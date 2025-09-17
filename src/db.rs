use rusqlite::{params, Connection};

use crate::sample::Sample;

pub fn init_db(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS samples (
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE NOT NULL,
            name TEXT NOT NULL,
            format TEXT,
            sample_rate INTEGER,
            size INTEGER
        );
        ",
    )?;
    Ok(())
}

pub fn insert_sample(conn: &Connection, meta: &Sample) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO samples (path, name, format, sample_rate, size)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            meta.path,
            meta.name,
            meta.format,
            meta.sample_rate,
            meta.size as i64,
        ],
    )?;
    Ok(())
}

pub fn load_samples(conn: &Connection) -> rusqlite::Result<Vec<Sample>> {
    let mut stmt = conn.prepare("SELECT id, path, name, format, sample_rate, size FROM samples")?;
    let rows = stmt.query_map([], |row| {
        Ok(Sample {
            id: row.get(0)?,
            path: row.get(1)?,
            name: row.get(2)?,
            format: row.get(3)?,
            sample_rate: row.get(4)?,
            size: row.get(5)?,
        })
    })?;

    let mut samples = Vec::new();
    for row in rows {
        samples.push(row?);
    }
    Ok(samples)
}
