//! # bolt402-core
//!
//! L402 client SDK core providing the protocol engine, token cache,
//! budget tracker, and Lightning backend abstraction.
//!
//! ## Architecture
//!
//! This crate follows hexagonal (ports & adapters) architecture:
//!
//! - **Domain**: Core types and business logic ([`budget::Budget`], [`receipt::Receipt`])
//! - **Ports**: Trait definitions for external dependencies ([`LnBackend`], [`port::TokenStore`])
//! - **Adapters**: In-memory implementations (see [`cache`] and [`budget`] modules)
//! - **Engine**: The [`L402Client`] orchestrates the full L402 flow
//!
//! External adapters (LND, CLN, etc.) live in separate crates.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use bolt402_core::{L402Client, L402ClientConfig};
//! use bolt402_core::budget::Budget;
//! use bolt402_core::cache::InMemoryTokenStore;
//! # use bolt402_core::port::{LnBackend, PaymentResult, NodeInfo};
//! # use bolt402_core::ClientError;
//! # use async_trait::async_trait;
//!
//! # struct MyLnd;
//! # #[async_trait]
//! # impl LnBackend for MyLnd {
//! #     async fn pay_invoice(&self, _: &str, _: u64) -> Result<PaymentResult, ClientError> { todo!() }
//! #     async fn get_balance(&self) -> Result<u64, ClientError> { todo!() }
//! #     async fn get_info(&self) -> Result<NodeInfo, ClientError> { todo!() }
//! # }
//!
//! # async fn example() {
//! let client = L402Client::builder()
//!     .ln_backend(MyLnd)
//!     .token_store(InMemoryTokenStore::default())
//!     .budget(Budget::unlimited())
//!     .build()
//!     .unwrap();
//!
//! let response = client.get("https://api.example.com/resource").await.unwrap();
//! println!("Status: {}", response.status());
//! # }
//! ```

/// Budget tracking for L402 payments with per-request, hourly, daily, and total limits.
pub mod budget;

/// In-memory LRU token cache.
pub mod cache;

/// L402 client engine: the main entry point for the SDK.
pub mod client;

/// Client error types.
pub mod error;

/// Port definitions (traits) for Lightning backends and token stores.
pub mod port;

/// Payment receipt types for audit and cost analysis.
pub mod receipt;

pub use client::{L402Client, L402ClientConfig, L402Response};
pub use error::ClientError;
pub use port::LnBackend;
