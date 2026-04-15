//! # l402-core
//!
//! L402 client SDK core providing the protocol engine, token cache,
//! budget tracker, and Lightning backend abstraction.

/// Budget tracking for L402 payments.
pub mod budget;

/// In-memory LRU token cache.
pub mod cache;

/// L402 client engine.
pub mod client;

/// Payment receipt types.
pub mod receipt;

pub use client::{L402Client, L402ClientConfig, L402Response};
