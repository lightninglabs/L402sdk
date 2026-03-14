//! # bolt402-lnd
//!
//! LND gRPC backend adapter for the bolt402 L402 client SDK.
//!
//! This crate implements the [`bolt402_core::LnBackend`] trait from `bolt402-core` using
//! LND's gRPC API, enabling the L402 client to pay invoices, query balances,
//! and retrieve node information through a connected LND node.
//!
//! ## Setup
//!
//! Connecting to LND requires three things:
//! - The gRPC endpoint address (must start with `https://`)
//! - A TLS certificate file (`tls.cert`)
//! - A macaroon file (typically `admin.macaroon` for payment capabilities)
//!
//! ## Example
//!
//! ```rust,no_run
//! use bolt402_lnd::LndBackend;
//! use bolt402_core::LnBackend;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let backend = LndBackend::connect(
//!     "https://localhost:10009",
//!     "/path/to/tls.cert",
//!     "/path/to/admin.macaroon",
//! ).await?;
//!
//! // Use with bolt402-core's L402Client:
//! // let client = L402Client::builder()
//! //     .ln_backend(backend)
//! //     .token_store(InMemoryTokenStore::default())
//! //     .build()?;
//!
//! let info = backend.get_info().await?;
//! println!("Connected to node: {} ({})", info.alias, info.pubkey);
//!
//! let balance = backend.get_balance().await?;
//! println!("Spendable balance: {} sats", balance);
//! # Ok(())
//! # }
//! ```
//!
//! ## Architecture
//!
//! This crate is an **adapter** in the hexagonal architecture pattern.
//! It depends on `bolt402-core` for the [`bolt402_core::LnBackend`] port trait and
//! implements it using `fedimint-tonic-lnd` for gRPC communication.
//!
//! ```text
//! bolt402-core (ports)
//!      ↑
//! bolt402-lnd (adapter: implements LnBackend via LND gRPC)
//! ```

mod backend;

pub use backend::LndBackend;
pub use backend::LndError;
