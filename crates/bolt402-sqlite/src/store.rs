//! SQLite-backed implementation of the [`TokenStore`] port.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rusqlite::Connection;

use bolt402_proto::ClientError;
use bolt402_proto::port::TokenStore;

use crate::SqliteStoreError;
use crate::migration;

/// A persistent token store backed by `SQLite`.
///
/// Stores L402 tokens in a `SQLite` database file so they survive process
/// restarts. Implements the [`TokenStore`] port from `bolt402-core`.
///
/// # Thread Safety
///
/// Uses `Arc<Mutex<Connection>>` with [`tokio::task::spawn_blocking`] to
/// avoid blocking the async runtime. The mutex is held only for the
/// duration of each SQL statement.
///
/// # TTL Support
///
/// Optionally assign a time-to-live to stored tokens. Expired tokens are
/// filtered out on [`get`](TokenStore::get) and can be purged with
/// [`cleanup_expired`](SqliteTokenStore::cleanup_expired).
///
/// # Example
///
/// ```rust,no_run
/// use bolt402_sqlite::SqliteTokenStore;
/// use bolt402_proto::port::TokenStore;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let store = SqliteTokenStore::new("tokens.db")?;
/// store.put("https://api.example.com", "mac", "pre").await?;
/// let token = store.get("https://api.example.com").await?;
/// assert!(token.is_some());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SqliteTokenStore {
    conn: Arc<Mutex<Connection>>,
    ttl: Option<Duration>,
}

impl SqliteTokenStore {
    /// Open or create a `SQLite` database at the given path.
    ///
    /// Creates the database file and schema if they don't exist.
    /// Runs any pending schema migrations.
    ///
    /// # Errors
    ///
    /// Returns [`SqliteStoreError::Open`] if the database cannot be opened,
    /// or [`SqliteStoreError::Migration`] if schema migration fails.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, SqliteStoreError> {
        let path = path.as_ref();
        let conn = Connection::open(path).map_err(|e| SqliteStoreError::Open {
            path: path.display().to_string(),
            source: e,
        })?;

        // Enable WAL mode for better concurrent read performance
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| SqliteStoreError::Migration(format!("failed to enable WAL mode: {e}")))?;

        migration::migrate(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            ttl: None,
        })
    }

    /// Create an in-memory `SQLite` store.
    ///
    /// Useful for testing. Data is lost when the store is dropped.
    ///
    /// # Errors
    ///
    /// Returns [`SqliteStoreError::Database`] if the in-memory database
    /// cannot be created.
    pub fn in_memory() -> Result<Self, SqliteStoreError> {
        let conn = Connection::open_in_memory()?;
        migration::migrate(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            ttl: None,
        })
    }

    /// Set the default TTL for new tokens.
    ///
    /// Tokens inserted after this call will have an `expires_at` timestamp
    /// computed as `now + ttl`. Expired tokens are excluded from
    /// [`get`](TokenStore::get) results.
    #[must_use]
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Remove all expired tokens from the database.
    ///
    /// Returns the number of tokens removed.
    ///
    /// # Errors
    ///
    /// Returns [`SqliteStoreError`] if the database operation fails.
    pub async fn cleanup_expired(&self) -> Result<u64, SqliteStoreError> {
        let conn = Arc::clone(&self.conn);
        spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SqliteStoreError::TaskJoin(format!("mutex poisoned: {e}")))?;

            let now = unix_now();
            let deleted = conn.execute(
                "DELETE FROM l402_tokens WHERE expires_at IS NOT NULL AND expires_at <= ?1",
                [now],
            )?;

            Ok(deleted as u64)
        })
        .await
    }

    /// Get the number of stored tokens (including expired ones).
    ///
    /// # Errors
    ///
    /// Returns [`SqliteStoreError`] if the database operation fails.
    pub async fn count(&self) -> Result<u64, SqliteStoreError> {
        let conn = Arc::clone(&self.conn);
        spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SqliteStoreError::TaskJoin(format!("mutex poisoned: {e}")))?;

            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM l402_tokens", [], |row| row.get(0))?;

            #[allow(clippy::cast_sign_loss)]
            Ok(count as u64)
        })
        .await
    }

    /// Compute the `expires_at` timestamp for a new token, if TTL is set.
    fn expires_at(&self) -> Option<i64> {
        self.ttl.map(|ttl| {
            let now = unix_now();
            now + i64::try_from(ttl.as_secs()).unwrap_or(i64::MAX)
        })
    }
}

#[async_trait]
impl TokenStore for SqliteTokenStore {
    async fn put(&self, endpoint: &str, macaroon: &str, preimage: &str) -> Result<(), ClientError> {
        let conn = Arc::clone(&self.conn);
        let endpoint = endpoint.to_string();
        let macaroon = macaroon.to_string();
        let preimage = preimage.to_string();
        let expires_at = self.expires_at();

        spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SqliteStoreError::TaskJoin(format!("mutex poisoned: {e}")))?;

            conn.execute(
                "INSERT INTO l402_tokens (endpoint, macaroon, preimage, created_at, expires_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(endpoint) DO UPDATE SET
                     macaroon = excluded.macaroon,
                     preimage = excluded.preimage,
                     created_at = excluded.created_at,
                     expires_at = excluded.expires_at",
                rusqlite::params![endpoint, macaroon, preimage, unix_now(), expires_at],
            )?;

            Ok(())
        })
        .await
        .map_err(ClientError::from)
    }

    async fn get(&self, endpoint: &str) -> Result<Option<(String, String)>, ClientError> {
        let conn = Arc::clone(&self.conn);
        let endpoint = endpoint.to_string();

        spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SqliteStoreError::TaskJoin(format!("mutex poisoned: {e}")))?;

            let now = unix_now();
            let result = conn.query_row(
                "SELECT macaroon, preimage FROM l402_tokens
                 WHERE endpoint = ?1
                   AND (expires_at IS NULL OR expires_at > ?2)",
                rusqlite::params![endpoint, now],
                |row| {
                    let macaroon: String = row.get(0)?;
                    let preimage: String = row.get(1)?;
                    Ok((macaroon, preimage))
                },
            );

            match result {
                Ok(pair) => Ok(Some(pair)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(SqliteStoreError::from(e)),
            }
        })
        .await
        .map_err(ClientError::from)
    }

    async fn remove(&self, endpoint: &str) -> Result<(), ClientError> {
        let conn = Arc::clone(&self.conn);
        let endpoint = endpoint.to_string();

        spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SqliteStoreError::TaskJoin(format!("mutex poisoned: {e}")))?;

            conn.execute("DELETE FROM l402_tokens WHERE endpoint = ?1", [&endpoint])?;

            Ok(())
        })
        .await
        .map_err(ClientError::from)
    }

    async fn clear(&self) -> Result<(), ClientError> {
        let conn = Arc::clone(&self.conn);

        spawn_blocking(move || {
            let conn = conn
                .lock()
                .map_err(|e| SqliteStoreError::TaskJoin(format!("mutex poisoned: {e}")))?;

            conn.execute("DELETE FROM l402_tokens", [])?;
            Ok(())
        })
        .await
        .map_err(ClientError::from)
    }
}

/// Run a blocking closure on the tokio blocking thread pool.
async fn spawn_blocking<F, T>(f: F) -> Result<T, SqliteStoreError>
where
    F: FnOnce() -> Result<T, SqliteStoreError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| SqliteStoreError::TaskJoin(e.to_string()))?
}

/// Get the current Unix timestamp in seconds.
#[allow(clippy::cast_possible_wrap)]
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn put_and_get() {
        let store = SqliteTokenStore::in_memory().unwrap();

        store
            .put("https://api.example.com/resource", "mac1", "pre1")
            .await
            .unwrap();

        let result = store.get("https://api.example.com/resource").await.unwrap();
        assert_eq!(result, Some(("mac1".to_string(), "pre1".to_string())));
    }

    #[tokio::test]
    async fn get_missing_returns_none() {
        let store = SqliteTokenStore::in_memory().unwrap();

        let result = store.get("https://api.example.com/missing").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn put_overwrites_existing() {
        let store = SqliteTokenStore::in_memory().unwrap();

        store
            .put("https://api.test.com", "mac1", "pre1")
            .await
            .unwrap();
        store
            .put("https://api.test.com", "mac2", "pre2")
            .await
            .unwrap();

        let result = store.get("https://api.test.com").await.unwrap();
        assert_eq!(result, Some(("mac2".to_string(), "pre2".to_string())));

        // Should still be one row
        assert_eq!(store.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn remove_deletes_token() {
        let store = SqliteTokenStore::in_memory().unwrap();

        store.put("https://a.com", "mac", "pre").await.unwrap();
        store.remove("https://a.com").await.unwrap();

        assert!(store.get("https://a.com").await.unwrap().is_none());
        assert_eq!(store.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn remove_nonexistent_is_ok() {
        let store = SqliteTokenStore::in_memory().unwrap();
        store.remove("https://nonexistent.com").await.unwrap();
    }

    #[tokio::test]
    async fn clear_removes_all() {
        let store = SqliteTokenStore::in_memory().unwrap();

        store.put("https://a.com", "mac1", "pre1").await.unwrap();
        store.put("https://b.com", "mac2", "pre2").await.unwrap();
        store.put("https://c.com", "mac3", "pre3").await.unwrap();

        assert_eq!(store.count().await.unwrap(), 3);

        store.clear().await.unwrap();
        assert_eq!(store.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn count_tracks_entries() {
        let store = SqliteTokenStore::in_memory().unwrap();

        assert_eq!(store.count().await.unwrap(), 0);

        store.put("https://a.com", "m", "p").await.unwrap();
        assert_eq!(store.count().await.unwrap(), 1);

        store.put("https://b.com", "m", "p").await.unwrap();
        assert_eq!(store.count().await.unwrap(), 2);

        store.remove("https://a.com").await.unwrap();
        assert_eq!(store.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn ttl_expiry_filters_on_get() {
        // Use a 1-second TTL
        let store = SqliteTokenStore::in_memory()
            .unwrap()
            .with_ttl(Duration::from_secs(1));

        store
            .put("https://api.example.com", "mac", "pre")
            .await
            .unwrap();

        // Should be available immediately
        assert!(
            store
                .get("https://api.example.com")
                .await
                .unwrap()
                .is_some()
        );

        // Wait for expiry
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should be filtered out
        assert!(
            store
                .get("https://api.example.com")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn no_ttl_means_no_expiry() {
        let store = SqliteTokenStore::in_memory().unwrap();

        store
            .put("https://api.example.com", "mac", "pre")
            .await
            .unwrap();

        // Without TTL, token should have no expires_at
        let result = store.get("https://api.example.com").await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn cleanup_expired_removes_stale_tokens() {
        let store = SqliteTokenStore::in_memory()
            .unwrap()
            .with_ttl(Duration::from_secs(1));

        store.put("https://a.com", "m1", "p1").await.unwrap();
        store.put("https://b.com", "m2", "p2").await.unwrap();

        // Wait for expiry
        tokio::time::sleep(Duration::from_secs(2)).await;

        let removed = store.cleanup_expired().await.unwrap();
        assert_eq!(removed, 2);
        assert_eq!(store.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn cleanup_expired_keeps_valid_tokens() {
        let store = SqliteTokenStore::in_memory()
            .unwrap()
            .with_ttl(Duration::from_secs(60));

        store.put("https://valid.com", "m1", "p1").await.unwrap();

        let removed = store.cleanup_expired().await.unwrap();
        assert_eq!(removed, 0);
        assert_eq!(store.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn file_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("tokens.db");

        // Create store and insert a token
        {
            let store = SqliteTokenStore::new(&db_path).unwrap();
            store
                .put("https://api.example.com", "mac", "pre")
                .await
                .unwrap();
        }

        // Reopen and verify persistence
        {
            let store = SqliteTokenStore::new(&db_path).unwrap();
            let result = store.get("https://api.example.com").await.unwrap();
            assert_eq!(result, Some(("mac".to_string(), "pre".to_string())));
        }
    }

    #[tokio::test]
    async fn concurrent_access() {
        let store = SqliteTokenStore::in_memory().unwrap();

        let mut handles = Vec::new();
        for i in 0..10 {
            let store = store.clone();
            handles.push(tokio::spawn(async move {
                let endpoint = format!("https://api{i}.example.com");
                let mac = format!("mac{i}");
                let pre = format!("pre{i}");

                store.put(&endpoint, &mac, &pre).await.unwrap();
                let result = store.get(&endpoint).await.unwrap();
                assert_eq!(result, Some((mac, pre)));
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        assert_eq!(store.count().await.unwrap(), 10);
    }

    #[tokio::test]
    async fn multiple_endpoints_independent() {
        let store = SqliteTokenStore::in_memory().unwrap();

        store.put("https://a.com", "mac_a", "pre_a").await.unwrap();
        store.put("https://b.com", "mac_b", "pre_b").await.unwrap();

        assert_eq!(
            store.get("https://a.com").await.unwrap(),
            Some(("mac_a".to_string(), "pre_a".to_string()))
        );
        assert_eq!(
            store.get("https://b.com").await.unwrap(),
            Some(("mac_b".to_string(), "pre_b".to_string()))
        );

        store.remove("https://a.com").await.unwrap();

        assert!(store.get("https://a.com").await.unwrap().is_none());
        assert!(store.get("https://b.com").await.unwrap().is_some());
    }
}
