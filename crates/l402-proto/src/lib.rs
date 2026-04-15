//! # l402-proto
//!
//! L402 protocol types, challenge parsing, and token construction.
//!
//! This crate provides the foundational types for the L402 protocol:
//! - [`L402Challenge`]: Parsed `WWW-Authenticate` header from a 402 response
//! - [`L402Token`]: Authorization token (macaroon + preimage) for authenticated requests
//! - [`L402Error`]: Protocol-level errors
//!
//! ## L402 Protocol Overview
//!
//! L402 (formerly LSAT) is an HTTP 402-based authentication protocol that uses
//! Lightning Network payments and macaroons for API access control:
//!
//! 1. Client sends a request to a protected resource
//! 2. Server responds with `HTTP 402` and a `WWW-Authenticate: L402` header
//! 3. Client pays the Lightning invoice from the challenge
//! 4. Client constructs an `Authorization: L402 <macaroon>:<preimage>` header
//! 5. Client retries the request with the authorization header
//!
//! ## Example
//!
//! ```rust
//! use l402_proto::{L402Challenge, L402Token};
//!
//! // Parse a challenge from a 402 response header
//! let header = r#"L402 macaroon="YWJjZGVm", invoice="lnbc100n1pj9nr7mpp5test""#;
//! let challenge = L402Challenge::from_header(header).unwrap();
//!
//! // After paying the invoice and obtaining the preimage:
//! let token = L402Token::new(challenge.macaroon.clone(), "abcdef1234567890".to_string());
//! let auth_header = token.to_header_value();
//! assert_eq!(auth_header, "L402 YWJjZGVm:abcdef1234567890");
//! ```

/// BOLT11 invoice amount decoding.
pub mod bolt11;

/// L402 challenge parsing from `WWW-Authenticate` headers.
pub mod challenge;

/// Client error types shared across all crates.
pub mod client_error;

/// Protocol-level error types.
pub mod error;

/// Port definitions (traits) for hexagonal architecture.
pub mod port;

/// L402 authorization token construction and parsing.
pub mod token;

pub use bolt11::{InvoiceAmount, decode_bolt11_amount};
pub use challenge::L402Challenge;
pub use client_error::ClientError;
pub use error::L402Error;
pub use port::{LnBackend, NodeInfo, PaymentResult, TokenStore};
pub use token::L402Token;
