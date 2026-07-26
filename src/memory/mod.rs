// SPDX-License-Identifier: Apache-2.0

//! Persistent memory — SQLite-based storage for learned associations.
//!
//! Stores:
//! - **Explained tokens**: explanation → token mappings
//! - **Phonetic corrections**: spoken text → corrected word mappings

use std::path::Path;

use rusqlite::{Connection, params};

use crate::error::{VoxKeyError, VoxKeyResult};

/// An explained token record.
#[derive(Debug, Clone)]
pub struct ExplainedRecord {
    /// Unique row ID
    pub id: i64,
    /// Language context (e.g., "prose", "code")
    pub language_context: String,
    /// The explanation text (hashed for lookup)
    pub explanation_hash: String,
    /// The explanation text (original)
    pub explanation_text: String,
    /// The resulting token
    pub token: String,
    /// When it was first stored (Unix timestamp)
    pub created_at: i64,
    /// How many times this has been used
    pub usage_count: i64,
    /// When it was last used
    pub last_used: i64,
}

/// A phonetic correction record.
#[derive(Debug, Clone)]
pub struct PhoneticCorrection {
    pub id: i64,
    pub spoken_text: String,
    pub corrected_token: String,
    pub language_context: String,
    pub created_at: i64,
    pub usage_count: i64,
}

/// Memory store backed by SQLite.
pub struct MemoryStore {
    conn: Connection,
}

impl MemoryStore {
    /// Open or create the database at the given path.
    pub fn open(path: &str) -> VoxKeyResult<Self> {
        // Ensure parent directory exists
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent).map_err(|_| {
                VoxKeyError::Database(rusqlite::Error::InvalidPath(parent.to_path_buf()))
            })?;
        }

        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.initialize_schema()?;
        log::info!("Memory store initialized at {path}");
        Ok(store)
    }

    /// Create an in-memory database for testing.
    pub fn open_in_memory() -> VoxKeyResult<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.initialize_schema()?;
        Ok(store)
    }

    /// Initialize database schema.
    fn initialize_schema(&self) -> VoxKeyResult<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS explained_tokens (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                language_context TEXT NOT NULL,
                explanation_hash TEXT NOT NULL UNIQUE,
                explanation_text TEXT NOT NULL,
                token           TEXT NOT NULL,
                created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                usage_count     INTEGER NOT NULL DEFAULT 1,
                last_used       INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );

            CREATE INDEX IF NOT EXISTS idx_explained_hash
                ON explained_tokens(explanation_hash);

            CREATE INDEX IF NOT EXISTS idx_explained_context
                ON explained_tokens(language_context);

            CREATE TABLE IF NOT EXISTS phonetic_corrections (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                spoken_text     TEXT NOT NULL,
                corrected_token TEXT NOT NULL,
                language_context TEXT NOT NULL,
                created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                usage_count     INTEGER NOT NULL DEFAULT 1
            );

            CREATE INDEX IF NOT EXISTS idx_phonetic_spoken
                ON phonetic_corrections(spoken_text);

            CREATE INDEX IF NOT EXISTS idx_phonetic_context
                ON phonetic_corrections(language_context);
            ",
        )?;
        Ok(())
    }

    // --- Explained tokens ---

    /// Look up an explained token by its explanation text.
    /// Automatically computes the hash internally.
    pub fn lookup_explained(
        &self,
        explanation_text: &str,
    ) -> VoxKeyResult<Option<ExplainedRecord>> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(explanation_text.as_bytes());
        let explanation_hash = hex::encode(&hasher.finalize()[..8]);
        let mut stmt = self.conn.prepare(
            "SELECT id, language_context, explanation_hash, explanation_text,
                    token, created_at, usage_count, last_used
             FROM explained_tokens
             WHERE explanation_hash = ?1",
        )?;

        let mut rows = stmt.query(params![explanation_hash])?;
        match rows.next()? {
            Some(row) => {
                let mut record = ExplainedRecord {
                    id: row.get(0)?,
                    language_context: row.get(1)?,
                    explanation_hash: row.get(2)?,
                    explanation_text: row.get(3)?,
                    token: row.get(4)?,
                    created_at: row.get(5)?,
                    usage_count: row.get(6)?,
                    last_used: row.get(7)?,
                };

                // Increment usage count
                record.usage_count += 1;
                self.conn.execute(
                    "UPDATE explained_tokens SET usage_count = ?1, last_used = strftime('%s', 'now')
                     WHERE id = ?2",
                    params![record.usage_count, record.id],
                )?;

                Ok(Some(record))
            }
            None => Ok(None),
        }
    }

    /// Store a new explained token mapping.
    pub fn store_explanation(
        &self,
        language_context: &str,
        explanation_text: &str,
        token: &str,
    ) -> VoxKeyResult<ExplainedRecord> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(explanation_text.as_bytes());
        let hash = hex::encode(&hasher.finalize()[..8]);

        self.conn.execute(
            "INSERT OR IGNORE INTO explained_tokens
             (language_context, explanation_hash, explanation_text, token)
             VALUES (?1, ?2, ?3, ?4)",
            params![language_context, hash, explanation_text, token],
        )?;

        Ok(ExplainedRecord {
            id: self.conn.last_insert_rowid(),
            language_context: language_context.to_string(),
            explanation_hash: hash,
            explanation_text: explanation_text.to_string(),
            token: token.to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            usage_count: 1,
            last_used: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        })
    }

    // --- Phonetic corrections ---

    /// Look up phonetic corrections for a spoken text.
    pub fn lookup_phonetic_correction(
        &self,
        spoken_text: &str,
    ) -> VoxKeyResult<Option<PhoneticCorrection>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, spoken_text, corrected_token, language_context,
                    created_at, usage_count
             FROM phonetic_corrections
             WHERE spoken_text = ?1
             ORDER BY usage_count DESC
             LIMIT 1",
        )?;

        let mut rows = stmt.query(params![spoken_text])?;
        match rows.next()? {
            Some(row) => Ok(Some(PhoneticCorrection {
                id: row.get(0)?,
                spoken_text: row.get(1)?,
                corrected_token: row.get(2)?,
                language_context: row.get(3)?,
                created_at: row.get(4)?,
                usage_count: row.get(5)?,
            })),
            None => Ok(None),
        }
    }

    /// Store a phonetic correction.
    pub fn store_phonetic_correction(
        &self,
        spoken_text: &str,
        corrected_token: &str,
        language_context: &str,
    ) -> VoxKeyResult<()> {
        // If there's already an entry for this spoken text, increment its count
        let updated = self.conn.execute(
            "UPDATE phonetic_corrections
             SET usage_count = usage_count + 1
             WHERE spoken_text = ?1 AND corrected_token = ?2",
            params![spoken_text, corrected_token],
        )?;

        if updated == 0 {
            self.conn.execute(
                "INSERT INTO phonetic_corrections (spoken_text, corrected_token, language_context)
                 VALUES (?1, ?2, ?3)",
                params![spoken_text, corrected_token, language_context],
            )?;
        }

        Ok(())
    }

    /// Get the total number of records.
    pub fn stats(&self) -> VoxKeyResult<(usize, usize)> {
        let explained: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM explained_tokens", [], |row| {
                    row.get(0)
                })?;
        let corrections: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM phonetic_corrections", [], |row| {
                    row.get(0)
                })?;
        Ok((explained as usize, corrections as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_store() -> MemoryStore {
        MemoryStore::open_in_memory().unwrap()
    }

    #[test]
    fn test_store_and_lookup_explanation() {
        let store = setup_store();
        store
            .store_explanation("prose", "a greeting", "hello")
            .unwrap();

        let result = store.lookup_explained("a greeting").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().token, "hello");
    }

    #[test]
    fn test_lookup_missing_explanation() {
        let store = setup_store();
        let result = store.lookup_explained("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_phonetic_correction() {
        let store = setup_store();
        store
            .store_phonetic_correction("helo", "hello", "prose")
            .unwrap();

        let result = store.lookup_phonetic_correction("helo").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().corrected_token, "hello");
    }

    #[test]
    fn test_phonetic_correction_increments_usage() {
        let store = setup_store();
        store
            .store_phonetic_correction("helo", "hello", "prose")
            .unwrap();
        store
            .store_phonetic_correction("helo", "hello", "prose")
            .unwrap();

        let result = store.lookup_phonetic_correction("helo").unwrap().unwrap();
        assert_eq!(result.usage_count, 2);
    }

    #[test]
    fn test_stats() {
        let store = setup_store();
        let (e, c) = store.stats().unwrap();
        assert_eq!(e, 0);
        assert_eq!(c, 0);

        store
            .store_explanation("prose", "test", "test_token")
            .unwrap();
        let (e, c) = store.stats().unwrap();
        assert_eq!(e, 1);
        assert_eq!(c, 0);
    }

    #[test]
    fn test_duplicate_explanation_is_ignored() {
        let store = setup_store();
        store
            .store_explanation("prose", "a greeting", "hello")
            .unwrap();
        store
            .store_explanation("prose", "a greeting", "hello")
            .unwrap();
        let (e, _) = store.stats().unwrap();
        assert_eq!(e, 1);
    }

    #[test]
    fn test_lookup_increments_usage() {
        let store = setup_store();
        store
            .store_explanation("prose", "a greeting", "hello")
            .unwrap();

        let r1 = store.lookup_explained("a greeting").unwrap().unwrap();
        let r2 = store.lookup_explained("a greeting").unwrap().unwrap();
        assert!(r2.usage_count >= r1.usage_count);
    }
}
