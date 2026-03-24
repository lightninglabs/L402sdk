//! Errors that can occur during L402 client operations.
//!
//! Defined in `bolt402-proto` so that adapter crates can use these error types
//! without depending on `bolt402-core` (which pulls in tokio/reqwest).

use crate::L402Error;

/// Errors that can occur during L402 client operations.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// L402 protocol error (challenge parsing, token construction).
    #[error("L402 protocol error: {0}")]
    Protocol(#[from] L402Error),

    /// Lightning payment failed.
    #[error("payment failed: {reason}")]
    PaymentFailed {
        /// Description of why the payment failed.
        reason: String,
    },

    /// Budget limit exceeded.
    #[error("budget exceeded: {reason}")]
    BudgetExceeded {
        /// Description of which budget limit was exceeded.
        reason: String,
    },

    /// The server did not return a valid L402 challenge with the 402 response.
    #[error("server returned 402 but no valid WWW-Authenticate header")]
    MissingChallenge,

    /// The invoice expired before payment could complete.
    #[error("invoice expired")]
    InvoiceExpired,

    /// The request was not retried successfully after payment.
    #[error("retry after payment failed: {reason}")]
    RetryFailed {
        /// Description of why the retry failed.
        reason: String,
    },

    /// Backend-specific error.
    #[error("backend error: {reason}")]
    Backend {
        /// Description of the backend error.
        reason: String,
    },

    /// HTTP request failed.
    #[error("HTTP error: {reason}")]
    Http {
        /// Description of the HTTP error.
        reason: String,
    },
}
