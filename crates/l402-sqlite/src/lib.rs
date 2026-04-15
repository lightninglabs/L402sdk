//! # l402-sqlite
//!
//! `SQLite` persistent token store for the L402sdk L402 client SDK.
//!
//! This crate provides [`SqliteTokenStore`], an implementation of the
//! [`l402_proto::port::TokenStore`] trait that persists L402 tokens
//! to a `SQLite` database. Tokens survive process restarts, preventing
//! unnecessary re-payments for resources the agent already has valid
//! credentials for.
//!
//! ## Architecture
//!
//! This is an **adapter** in the hexagonal architecture. It depends on
//! `l402-proto` for the [`TokenStore`](l402_proto::port::TokenStore) port
//! definition and [`ClientError`](l402_proto::ClientError) type.
//!
//! Uses `rusqlite` with the `bundled` feature so no system `SQLite` library
//! is required.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use l402_sqlite::SqliteTokenStore;
//! use l402_proto::port::TokenStore;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Open (or create) a database file
//! let store = SqliteTokenStore::new("tokens.db")?;
//!
//! // Use it like any other TokenStore
//! store.put("https://api.example.com/data", "macaroon_b64", "preimage_hex").await?;
//!
//! if let Some((mac, pre)) = store.get("https://api.example.com/data").await? {
//!     println!("Cached token: macaroon={mac}, preimage={pre}");
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## TTL Support
//!
//! Tokens can be assigned a time-to-live. Expired tokens are filtered out
//! on read and can be purged with [`SqliteTokenStore::cleanup_expired`].
//!
//! ```rust,no_run
//! use l402_sqlite::SqliteTokenStore;
//! use std::time::Duration;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let store = SqliteTokenStore::new("tokens.db")?
//!     .with_ttl(Duration::from_secs(3600)); // 1-hour TTL
//! # Ok(())
//! # }
//! ```

mod error;
mod migration;
mod store;

pub use error::SqliteStoreError;
pub use store::SqliteTokenStore;
