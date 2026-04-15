# Design: SQLite Persistent Token Store

**Issue:** #47
**Author:** Dario Anongba Varela
**Date:** 2026-03-21
**Status:** Proposed

## Problem

The only `TokenStore` implementation today is `InMemoryTokenStore`, which loses all cached L402 tokens when the process exits. For production AI agents running long-lived tasks with periodic restarts (cron jobs, container rescheduling, crashes), this means:

1. **Wasted payments** — agents re-pay for resources they already have valid tokens for
2. **Unnecessary latency** — every restart triggers full L402 negotiation instead of cache hits
3. **Poor cost tracking** — no durable record of which tokens are active

A persistent store is listed in the original proposal as a Month 5 deliverable.

## Proposed Design

### New Crate: `l402-sqlite`

A dedicated crate rather than a feature flag in `l402-core` because:
- Keeps `l402-core` dependency-free (no `rusqlite` in the core)
- Follows the hexagonal architecture: `SqliteTokenStore` is an adapter
- Users who don't need persistence don't pull in SQLite

### Schema

```sql
CREATE TABLE IF NOT EXISTS l402_tokens (
    endpoint TEXT PRIMARY KEY,
    macaroon TEXT NOT NULL,
    preimage TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    expires_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_l402_tokens_expires_at
    ON l402_tokens(expires_at);
```

Single table, `endpoint` as primary key (matching `InMemoryTokenStore` semantics). Optional `expires_at` for TTL support.

### API

```rust
use l402_proto::port::TokenStore;

pub struct SqliteTokenStore { /* ... */ }

impl SqliteTokenStore {
    /// Open or create a SQLite database at the given path.
    /// Creates the schema if it doesn't exist.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, SqliteStoreError>;

    /// Create an in-memory SQLite store (useful for testing).
    pub fn in_memory() -> Result<Self, SqliteStoreError>;

    /// Set the default TTL for new tokens.
    /// Tokens older than this are treated as expired on `get`.
    pub fn with_ttl(self, ttl: Duration) -> Self;

    /// Remove all expired tokens from the database.
    pub async fn cleanup_expired(&self) -> Result<u64, SqliteStoreError>;

    /// Get the number of stored tokens.
    pub async fn count(&self) -> Result<u64, SqliteStoreError>;
}

#[async_trait]
impl TokenStore for SqliteTokenStore {
    async fn put(&self, endpoint: &str, macaroon: &str, preimage: &str) -> Result<(), ClientError>;
    async fn get(&self, endpoint: &str) -> Result<Option<(String, String)>, ClientError>;
    async fn remove(&self, endpoint: &str) -> Result<(), ClientError>;
    async fn clear(&self) -> Result<(), ClientError>;
}
```

### Thread Safety

`rusqlite::Connection` is `!Send`, which conflicts with `async_trait` requirements. Solutions:

**Chosen approach: `tokio::task::spawn_blocking` with `Arc<Mutex<Connection>>`**

- Wrap `Connection` in `Arc<Mutex<>>` (std mutex, not tokio — held briefly)
- All DB operations run inside `spawn_blocking` to avoid blocking the async runtime
- Simple, correct, and sufficient for the access patterns (low contention — token operations are infrequent)

Alternative considered: `r2d2` connection pool or `deadpool-sqlite`. Overkill for single-file SQLite with low concurrency. Can upgrade later if needed.

### Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum SqliteStoreError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("failed to open database at {path}: {source}")]
    Open { path: String, source: rusqlite::Error },

    #[error("schema migration failed: {0}")]
    Migration(String),
}
```

`SqliteStoreError` converts to `ClientError::Backend` when used through the `TokenStore` trait.

### Migration Strategy

Schema version tracked via SQLite `user_version` pragma:
- Version 0 → create initial schema
- Future versions → ALTER TABLE or new tables

```rust
fn migrate(conn: &Connection) -> Result<(), SqliteStoreError> {
    let version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version < 1 {
        conn.execute_batch(SCHEMA_V1)?;
        conn.pragma_update(None, "user_version", 1)?;
    }
    // Future: if version < 2 { ... }
    Ok(())
}
```

## Key Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Separate crate vs feature flag | Separate crate | Keeps core dependency-free, follows hexagonal pattern |
| `rusqlite` vs `sqlx` | `rusqlite` | Synchronous API is simpler, no need for async SQL driver overhead |
| Connection strategy | `Arc<Mutex<Connection>>` + `spawn_blocking` | Simple, correct, low contention |
| TTL | Optional, applied on read | Avoids background tasks; expired tokens filtered on `get` |
| Schema migration | `user_version` pragma | Standard SQLite pattern, no external migration tool needed |

## Alternatives Considered

1. **Feature flag on `l402-core`**: Would keep it in one crate but forces `rusqlite` as an optional dependency on the core. Violates clean architecture — the core shouldn't know about specific storage technologies.

2. **`sled` embedded DB**: Faster for some patterns but less mature, no SQL, harder to inspect manually. SQLite is battle-tested and inspectable with standard tools.

3. **JSON file store**: Simplest possible persistence but no concurrency safety, no indexing, poor performance at scale. Not suitable for production.

## Testing Plan

1. **Unit tests** (in `l402-sqlite`):
   - `new` creates DB and schema
   - `in_memory` works for tests
   - All `TokenStore` trait methods (put/get/remove/clear)
   - TTL expiry behavior
   - Overwrite existing endpoint
   - `cleanup_expired` removes stale tokens
   - `count` returns correct value
   - Schema migration from version 0 → 1

2. **Concurrency tests**:
   - Multiple concurrent `put`/`get` operations
   - No deadlocks under contention

3. **Integration tests** (in `tests/`):
   - `L402Client` with `SqliteTokenStore` against `l402-mock` server
   - Token survives "restart" (drop client, recreate with same DB path)

## Dependencies

- `rusqlite` (with `bundled` feature for zero-system-dependency builds)
- `l402-proto` (for `TokenStore` trait and `ClientError`)
- `tokio` (for `spawn_blocking`)
- `thiserror` (for error types)
