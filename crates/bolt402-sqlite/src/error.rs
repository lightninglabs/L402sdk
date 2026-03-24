//! Error types for the `SQLite` token store.

use bolt402_proto::ClientError;

/// Errors specific to the `SQLite` token store.
#[derive(Debug, thiserror::Error)]
pub enum SqliteStoreError {
    /// A database operation failed.
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// Failed to open or create the database file.
    #[error("failed to open database at {path}: {source}")]
    Open {
        /// The path that was attempted.
        path: String,
        /// The underlying `SQLite` error.
        source: rusqlite::Error,
    },

    /// Schema migration failed.
    #[error("schema migration failed: {0}")]
    Migration(String),

    /// A blocking task was cancelled or panicked.
    #[error("blocking task failed: {0}")]
    TaskJoin(String),
}

impl From<SqliteStoreError> for ClientError {
    fn from(err: SqliteStoreError) -> Self {
        ClientError::Backend {
            reason: err.to_string(),
        }
    }
}
