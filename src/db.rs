use rusqlite::{Connection, params};

use crate::sample::Sample;

pub struct DB {
    pub conn: Connection,
}
impl DB {
    pub fn new() -> Self {
        let conn = Connection::open("samples.db").expect("failed to open db");

        Self::init_db(&conn).expect("failed to initialize db");

        Self { conn }
    }

    fn init_db(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS samples (
                id INTEGER PRIMARY KEY,
                path TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL,
                format TEXT,
                sample_count INTEGER,
                sample_rate INTEGER,
                key TEXT,
                size INTEGER,
                rating INTEGER,
                notes TEXT
            );
        ",
        )?;
        Ok(())
    }

    pub fn insert_sample(&self, meta: &Sample) -> rusqlite::Result<()> {
        self.conn.execute(
            "
            INSERT OR IGNORE INTO 
                samples (
                    path,
                    name,
                    format,
                    sample_count,
                    sample_rate,
                    key, 
                    size,
                    rating,
                    notes
                )
            VALUES 
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                meta.path,
                meta.name,
                meta.format,
                meta.sample_count,
                meta.sample_rate,
                meta.key,
                meta.size as i64,
                meta.rating,
                meta.notes,
            ],
        )?;
        Ok(())
    }

    pub fn update_rating(&self, sample_id: isize, rating: u8) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE samples SET rating = ?1 WHERE id = ?2",
            params![rating, sample_id],
        )?;
        Ok(())
    }

    pub fn get_samples(&self) -> rusqlite::Result<Vec<Sample>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT 
                id, 
                path, 
                name,
                format,
                sample_count,
                sample_rate,
                key,
                size,
                rating,
                notes 
            FROM 
                samples",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Sample {
                id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
                format: row.get(3)?,
                sample_count: row.get(4)?,
                sample_rate: row.get(5)?,
                key: row.get(6)?,
                size: row.get(7)?,
                rating: row.get(8)?,
                notes: row.get(9)?,
            })
        })?;

        let mut samples = Vec::new();
        for row in rows {
            samples.push(row?);
        }
        Ok(samples)
    }
}
