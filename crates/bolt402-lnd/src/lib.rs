//! # bolt402-lnd
//!
//! LND backend adapters for the bolt402 L402 client SDK.
//!
//! This crate provides two implementations of [`bolt402_proto::LnBackend`]:
//!
//! - **gRPC** (feature `grpc`, enabled by default): Uses LND's gRPC API via
//!   `tonic` with vendored proto files. Requires a TLS certificate and
//!   macaroon file.
//!
//! - **REST** (feature `rest`): Uses LND's REST API via `reqwest`. Simpler
//!   to configure (no proto files), works in WASM/browser environments, and
//!   only requires a hex-encoded macaroon.
//!
//! Both features can be enabled simultaneously.
//!
//! ## gRPC Example
//!
//! ```rust,no_run
//! # #[cfg(feature = "grpc")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use bolt402_lnd::LndGrpcBackend;
//! use bolt402_proto::LnBackend;
//!
//! let backend = LndGrpcBackend::connect(
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
//! ## REST Example
//!
//! ```rust,no_run
//! # #[cfg(feature = "rest")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use bolt402_lnd::LndRestBackend;
//! use bolt402_proto::LnBackend;
//!
//! let backend = LndRestBackend::new(
//!     "https://localhost:8080",
//!     "0201036c6e640258030a...",
//! )?;
//!
//! let info = backend.get_info().await?;
//! println!("Connected to: {} ({})", info.alias, info.pubkey);
//! # Ok(())
//! # }
//! ```
//!
//! ## Architecture
//!
//! This crate is an adapter in the hexagonal architecture. Both backends
//! implement the [`bolt402_proto::LnBackend`] port trait.

pub mod error;

#[cfg(feature = "grpc")]
mod grpc;

#[cfg(feature = "rest")]
mod rest;

/// Generated LND Lightning gRPC types.
#[cfg(feature = "grpc")]
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
#[cfg(feature = "grpc")]
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

// Re-exports
pub use error::LndError;

#[cfg(feature = "grpc")]
pub use grpc::LndGrpcBackend;

#[cfg(feature = "rest")]
pub use rest::LndRestBackend;

// Backwards compatibility: when only grpc is enabled, also export as LndBackend
#[cfg(all(feature = "grpc", not(feature = "rest")))]
pub use grpc::LndGrpcBackend as LndBackend;
