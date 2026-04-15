//! # l402-nwc
//!
//! Nostr Wallet Connect ([NIP-47](https://github.com/nostr-protocol/nips/blob/master/47.md))
//! Lightning backend for the L402sdk L402 client SDK.
//!
//! This crate implements the [`l402_proto::port::LnBackend`] trait using the
//! NWC protocol, enabling L402sdk to pay L402 invoices through any
//! NWC-compatible wallet without direct Lightning node access.
//!
//! ## Supported Wallets
//!
//! - [Alby Hub](https://albyhub.com/)
//! - [Mutiny Wallet](https://www.mutinywallet.com/)
//! - [LNbits](https://lnbits.com/)
//! - [Phoenixd](https://phoenix.acinq.co/)
//! - Any wallet implementing NIP-47
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use l402_nwc::NwcBackend;
//! use l402_core::{L402Client, L402ClientConfig};
//! use l402_core::budget::Budget;
//! use l402_core::cache::InMemoryTokenStore;
//!
//! # async fn example() {
//! // Connect via NWC URI (get this from your wallet)
//! let backend = NwcBackend::new("nostr+walletconnect://...").await.unwrap();
//!
//! let client = L402Client::builder()
//!     .ln_backend(backend)
//!     .token_store(InMemoryTokenStore::default())
//!     .budget(Budget::unlimited())
//!     .build()
//!     .unwrap();
//!
//! // Make L402-gated requests — payments happen automatically
//! let response = client.get("https://api.example.com/resource").await.unwrap();
//! println!("Status: {}", response.status());
//! # }
//! ```
//!
//! ## Environment Variable
//!
//! You can also configure via environment variable:
//!
//! ```rust,no_run
//! use l402_nwc::NwcBackend;
//!
//! # async fn example() {
//! // Reads NWC_CONNECTION_URI from the environment
//! let backend = NwcBackend::from_env().await.unwrap();
//! # }
//! ```

#![warn(missing_docs)]

mod backend;
mod error;

pub use backend::NwcBackend;
pub use error::NwcError;
