//! # bolt402-lnd
//!
//! LND gRPC backend adapter for the bolt402 L402 client SDK.
//!
//! This crate implements the [`bolt402_core::LnBackend`] trait using
//! LND's gRPC API, enabling the L402 client to pay invoices, query balances,
//! and retrieve node information through a connected LND node.
//!
//! ## Setup
//!
//! Connecting to LND requires:
//! - gRPC endpoint (e.g. `https://localhost:10009`)
//! - TLS certificate (`tls.cert`)
//! - Admin macaroon (`admin.macaroon`)
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
//! let info = backend.get_info().await?;
//! println!("Connected to: {} ({})", info.alias, info.pubkey);
//! # Ok(())
//! # }
//! ```
//!
//! ## Architecture
//!
//! This crate is an adapter in the hexagonal architecture. It depends on
//! `bolt402-core` for the [`bolt402_core::LnBackend`] port trait and uses
//! `tonic` with vendored LND proto files for gRPC communication.

mod backend;

/// Generated LND Lightning gRPC types.
#[allow(
    clippy::all,
    clippy::pedantic,
    missing_docs,
    unused_qualifications,
    unreachable_pub,
    rustdoc::invalid_html_tags
)]
pub mod lnrpc {
    tonic::include_proto!("lnrpc");
}

/// Generated LND Router gRPC types.
#[allow(
    clippy::all,
    clippy::pedantic,
    missing_docs,
    unused_qualifications,
    unreachable_pub,
    rustdoc::invalid_html_tags
)]
pub mod routerrpc {
    tonic::include_proto!("routerrpc");
}

pub use backend::LndBackend;
pub use backend::LndError;
