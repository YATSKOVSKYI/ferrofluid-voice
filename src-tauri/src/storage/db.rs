use crate::errors::AppError;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveTranscriptInput {
    pub text: String,
    pub language: String,
    pub duration: f64,
    pub model_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: i64,
    pub text: String,
    pub language: String,
    pub created_at: String,
    pub duration_seconds: f64,
    pub model_name: String,
}

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let connection = Connection::open(path)?;
        let db = Self { connection };
        db.migrate()?;
        Ok(db)
    }

    pub fn save_transcript(&self, input: SaveTranscriptInput) -> Result<i64, AppError> {
        let created_at: DateTime<Utc> = Utc::now();
        self.connection.execute(
            "INSERT INTO transcripts (text, language, created_at, duration_seconds, model_name)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                input.text,
                input.language,
                created_at.to_rfc3339(),
                input.duration,
                input.model_name
            ],
        )?;

        Ok(self.connection.last_insert_rowid())
    }

    pub fn history(&self) -> Result<Vec<HistoryItem>, AppError> {
        let mut statement = self.connection.prepare(
            "SELECT id, text, language, created_at, duration_seconds, model_name
             FROM transcripts
             ORDER BY datetime(created_at) DESC, id DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(HistoryItem {
                id: row.get(0)?,
                text: row.get(1)?,
                language: row.get(2)?,
                created_at: row.get(3)?,
                duration_seconds: row.get(4)?,
                model_name: row.get(5)?,
            })
        })?;

        let mut items = Vec::new();
        for item in rows {
            items.push(item?);
        }
        Ok(items)
    }

    pub fn delete_history_item(&self, id: i64) -> Result<(), AppError> {
        self.connection
            .execute("DELETE FROM transcripts WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn migrate(&self) -> Result<(), AppError> {
        self.connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS transcripts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text TEXT NOT NULL,
                language TEXT NOT NULL,
                created_at TEXT NOT NULL,
                duration_seconds REAL NOT NULL,
                model_name TEXT NOT NULL
            );",
        )?;
        Ok(())
    }
}
