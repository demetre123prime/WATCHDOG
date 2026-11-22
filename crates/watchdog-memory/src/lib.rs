use rusqlite::{params, Connection, Result};
use std::path::Path;

/// Local text-only observations. This store never receives screenshot bytes.
pub struct MemoryStore {
    connection: Connection,
}

impl MemoryStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS observations (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 note TEXT NOT NULL,
                 created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );",
        )?;
        Ok(Self { connection })
    }

    pub fn save_observation(&self, note: &str) -> Result<()> {
        let note = note.trim();
        if note.is_empty() {
            return Ok(());
        }
        self.connection.execute(
            "INSERT INTO observations (note) VALUES (?1)",
            params![note],
        )?;
        Ok(())
    }

    pub fn list_observations(&self) -> Result<Vec<String>> {
        let mut statement = self.connection.prepare(
            "SELECT note FROM observations ORDER BY id DESC LIMIT 100",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect()
    }

    pub fn clear(&self) -> Result<()> {
        self.connection.execute("DELETE FROM observations", [])?;
        Ok(())
    }
}
