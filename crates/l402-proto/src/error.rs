//! Errors that can occur during L402 protocol operations.

use thiserror::Error;

/// Errors that can occur during L402 protocol operations.
#[derive(Debug, Error)]
pub enum L402Error {
    /// The `WWW-Authenticate` header is missing or malformed.
    #[error("invalid L402 challenge: {reason}")]
    InvalidChallenge {
        /// Description of why the challenge is invalid.
        reason: String,
    },

    /// The macaroon in the challenge could not be decoded.
    #[error("invalid macaroon: {reason}")]
    InvalidMacaroon {
        /// Description of the macaroon decoding failure.
        reason: String,
    },

    /// The invoice in the challenge is invalid or expired.
    #[error("invalid invoice: {reason}")]
    InvalidInvoice {
        /// Description of why the invoice is invalid.
        reason: String,
    },

    /// The preimage does not match the payment hash.
    #[error("preimage mismatch: expected {expected}, got {actual}")]
    PreimageMismatch {
        /// Expected preimage hash.
        expected: String,
        /// Actual preimage hash received.
        actual: String,
    },

    /// The L402 token could not be constructed.
    #[error("invalid token: {reason}")]
    InvalidToken {
        /// Description of why the token is invalid.
        reason: String,
    },

    /// Base64 decoding failed.
    #[error("base64 decode error: {0}")]
    Base64Decode(#[from] base64::DecodeError),
}
