//! Error types for the LND backend adapters.

use bolt402_proto::ClientError;

/// Error type specific to the LND backend.
#[derive(Debug, thiserror::Error)]
pub enum LndError {
    /// HTTP or gRPC transport error.
    #[error("LND transport error: {0}")]
    Transport(String),

    /// gRPC call returned an error status.
    #[cfg(feature = "grpc")]
    #[error("LND gRPC error: {0}")]
    Rpc(#[from] tonic::Status),

    /// REST API returned an error.
    #[cfg(feature = "rest")]
    #[error("LND REST API error ({status}): {body}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Response body.
        body: String,
    },

    /// Failed to read TLS certificate or macaroon file.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Payment failed with a specific reason.
    #[error("payment failed: {0}")]
    Payment(String),

    /// Failed to deserialize a response.
    #[cfg(feature = "rest")]
    #[error("deserialization error: {0}")]
    Deserialize(String),
}

#[cfg(feature = "grpc")]
impl From<tonic::transport::Error> for LndError {
    fn from(err: tonic::transport::Error) -> Self {
        Self::Transport(err.to_string())
    }
}

#[cfg(feature = "rest")]
impl From<reqwest::Error> for LndError {
    fn from(err: reqwest::Error) -> Self {
        Self::Transport(err.to_string())
    }
}

impl From<LndError> for ClientError {
    fn from(err: LndError) -> Self {
        match err {
            LndError::Payment(reason) => ClientError::PaymentFailed { reason },
            other => ClientError::Backend {
                reason: other.to_string(),
            },
        }
    }
}
