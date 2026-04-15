//! Schema migration for the `SQLite` token store.
//!
//! Uses `SQLite`'s `user_version` pragma to track schema versions.
//! Each version bump applies an incremental migration.

use rusqlite::Connection;

use crate::SqliteStoreError;

/// SQL for the initial schema (version 1).
const SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS l402_tokens (
    endpoint  TEXT PRIMARY KEY,
    macaroon  TEXT NOT NULL,
    preimage  TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    expires_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_l402_tokens_expires_at
    ON l402_tokens(expires_at);
";

/// Run all pending migrations on the given connection.
///
/// This is idempotent: calling it multiple times on an already-migrated
/// database is a no-op.
pub(crate) fn migrate(conn: &Connection) -> Result<(), SqliteStoreError> {
    let version: i32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| SqliteStoreError::Migration(format!("failed to read user_version: {e}")))?;

    if version < 1 {
        conn.execute_batch(SCHEMA_V1)
            .map_err(|e| SqliteStoreError::Migration(format!("v1 migration failed: {e}")))?;

        conn.pragma_update(None, "user_version", 1).map_err(|e| {
            SqliteStoreError::Migration(format!("failed to update user_version: {e}"))
        })?;

        tracing::info!("l402-sqlite: migrated schema to version 1");
    }

    // Future migrations: if version < 2 { ... }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_creates_schema() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();

        // Verify the table exists by querying it
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM l402_tokens", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);

        // Verify user_version was set
        let version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn migrate_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap(); // Should not fail

        let version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn index_exists_after_migration() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();

        let index_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_l402_tokens_expires_at'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(index_count, 1);
    }
}
