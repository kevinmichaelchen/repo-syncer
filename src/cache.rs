use crate::types::Fork;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: i32 = 1;

/// `SQLite` cache for fork metadata.
pub struct Cache {
    conn: Connection,
}

impl Cache {
    /// Open or create the cache database.
    pub fn open() -> Result<Self> {
        let path = Self::cache_path()?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create cache directory")?;
        }

        let conn = Connection::open(&path)
            .context("Failed to open cache database")?;

        let cache = Self { conn };
        cache.init_schema()?;

        Ok(cache)
    }

    /// Get the path to the cache database.
    pub fn cache_path() -> Result<PathBuf> {
        let cache_dir = dirs::cache_dir()
            .context("Could not determine cache directory")?;
        Ok(cache_dir.join("repo-syncer").join("forks.db"))
    }

    /// Initialize the database schema.
    fn init_schema(&self) -> Result<()> {
        // Check schema version
        let version = self.get_metadata("schema_version")
            .unwrap_or(None)
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);

        if version < SCHEMA_VERSION {
            // Create or migrate schema
            self.conn.execute_batch(
                r"
                CREATE TABLE IF NOT EXISTS forks (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    owner TEXT NOT NULL,
                    parent_owner TEXT NOT NULL,
                    parent_name TEXT NOT NULL,
                    default_branch TEXT NOT NULL,
                    description TEXT,
                    primary_language TEXT,
                    created_at TEXT,
                    updated_at TEXT,
                    fetched_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS metadata (
                    key TEXT PRIMARY KEY,
                    value TEXT
                );

                CREATE INDEX IF NOT EXISTS idx_forks_fetched_at ON forks(fetched_at);
                CREATE INDEX IF NOT EXISTS idx_forks_created_at ON forks(created_at);
                "
            ).context("Failed to create schema")?;

            self.set_metadata("schema_version", &SCHEMA_VERSION.to_string())?;
        }

        Ok(())
    }

    /// Load all forks from the cache.
    pub fn load_forks(&self, tool_home: &Path) -> Result<Vec<Fork>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, owner, parent_owner, parent_name, default_branch,
                    description, primary_language, created_at, updated_at
             FROM forks
             ORDER BY created_at DESC NULLS LAST"
        )?;

        let forks = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            let owner: String = row.get(2)?;
            let parent_owner: String = row.get(3)?;
            let parent_name: String = row.get(4)?;
            let default_branch: String = row.get(5)?;
            let description: Option<String> = row.get(6)?;
            let primary_language: Option<String> = row.get(7)?;
            let created_at: Option<String> = row.get(8)?;
            let updated_at: Option<String> = row.get(9)?;

            let local_path = tool_home.join(&owner).join(&name);
            let is_cloned = local_path.exists();

            Ok(Fork {
                name,
                owner,
                parent_owner,
                parent_name,
                default_branch,
                local_path,
                is_cloned,
                description,
                primary_language,
                created_at: created_at.and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                updated_at: updated_at.and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(forks)
    }

    /// Save multiple forks to the cache.
    pub fn save_forks(&self, forks: &[Fork]) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        for fork in forks {
            self.conn.execute(
                "INSERT OR REPLACE INTO forks
                 (id, name, owner, parent_owner, parent_name, default_branch,
                  description, primary_language, created_at, updated_at, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    format!("{}/{}", fork.owner, fork.name),
                    fork.name,
                    fork.owner,
                    fork.parent_owner,
                    fork.parent_name,
                    fork.default_branch,
                    fork.description,
                    fork.primary_language,
                    fork.created_at.map(|dt| dt.to_rfc3339()),
                    fork.updated_at.map(|dt| dt.to_rfc3339()),
                    now,
                ],
            )?;
        }

        Ok(())
    }

    /// Save a single fork to the cache.
    pub fn save_fork(&self, fork: &Fork) -> Result<()> {
        self.save_forks(std::slice::from_ref(fork))
    }

    /// Remove a fork from the cache.
    pub fn remove_fork(&self, owner: &str, name: &str) -> Result<()> {
        let id = format!("{owner}/{name}");
        self.conn.execute("DELETE FROM forks WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Check if a fork exists in the cache.
    pub fn has_fork(&self, owner: &str, name: &str) -> Result<bool> {
        let id = format!("{owner}/{name}");
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM forks WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get the timestamp of the last full sync.
    pub fn last_full_sync(&self) -> Result<Option<DateTime<Utc>>> {
        self.get_metadata("last_full_sync")?
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| Ok(dt.with_timezone(&Utc)))
            .transpose()
    }

    /// Set the timestamp of the last full sync.
    pub fn set_last_full_sync(&self, when: DateTime<Utc>) -> Result<()> {
        self.set_metadata("last_full_sync", &when.to_rfc3339())
    }

    /// Get a metadata value.
    fn get_metadata(&self, key: &str) -> Result<Option<String>> {
        let result = self.conn.query_row(
            "SELECT value FROM metadata WHERE key = ?1",
            params![key],
            |row| row.get(0),
        );

        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Set a metadata value.
    fn set_metadata(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM forks",
            [],
            |row| row.get(0),
        )?;
        Ok(count == 0)
    }

    /// Get the number of cached forks.
    pub fn fork_count(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM forks",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_fork() -> Fork {
        Fork {
            name: "test-repo".to_string(),
            owner: "testuser".to_string(),
            parent_owner: "upstream".to_string(),
            parent_name: "test-repo".to_string(),
            default_branch: "main".to_string(),
            local_path: PathBuf::from("/tmp/test"),
            is_cloned: false,
            description: Some("A test repo".to_string()),
            primary_language: Some("Rust".to_string()),
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        }
    }

    #[test]
    fn test_cache_roundtrip() {
        // Use in-memory database for testing
        let conn = Connection::open_in_memory().unwrap();
        let cache = Cache { conn };
        cache.init_schema().unwrap();

        let fork = test_fork();
        cache.save_fork(&fork).unwrap();

        let forks = cache.load_forks(Path::new("/tmp")).unwrap();
        assert_eq!(forks.len(), 1);
        assert_eq!(forks[0].name, "test-repo");
        assert_eq!(forks[0].owner, "testuser");
    }
}
