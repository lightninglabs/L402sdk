//! # bolt402-swissknife
//!
//! SwissKnife REST API backend adapter for the bolt402 L402 client SDK.
//!
//! This crate implements the [`bolt402_proto::LnBackend`] trait using
//! Numeraire SwissKnife's REST API, enabling the L402 client to pay invoices,
//! query balances, and retrieve wallet information through a custodial
//! SwissKnife instance.
//!
//! ## Setup
//!
//! Connecting to SwissKnife requires:
//! - Base URL of the SwissKnife instance (e.g. `https://app.numeraire.tech`)
//! - API key with `read:transaction` and `write:transaction` permissions
//!
//! ## Example
//!
//! ```rust,no_run
//! use bolt402_swissknife::SwissKnifeBackend;
//! use bolt402_proto::LnBackend;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Connect with explicit credentials
//! let backend = SwissKnifeBackend::new(
//!     "https://app.numeraire.tech",
//!     "your-api-key",
//! );
//!
//! let balance = backend.get_balance().await?;
//! println!("Balance: {} sats", balance);
//!
//! // Or use environment variables
//! let backend = SwissKnifeBackend::from_env()?;
//! let info = backend.get_info().await?;
//! println!("Wallet: {}", info.alias);
//! # Ok(())
//! # }
//! ```
//!
//! ## Architecture
//!
//! This crate is an adapter in the hexagonal architecture. It depends on
//! `bolt402-core` for the [`bolt402_proto::LnBackend`] port trait and uses
//! `reqwest` for HTTP communication with the SwissKnife REST API.
//!
//! ## SwissKnife vs LND
//!
//! | Aspect | SwissKnife | LND |
//! |--------|-----------|-----|
//! | Model | Custodial | Self-custodial |
//! | Protocol | REST/JSON | gRPC/protobuf |
//! | Auth | API key | TLS + macaroon |
//! | Setup | Create account + API key | Run node + sync chain |
//! | Fee control | No (provider-managed) | Yes (max_fee_sats) |

mod backend;
pub(crate) mod types;

pub use backend::SwissKnifeBackend;
pub use backend::SwissKnifeError;
