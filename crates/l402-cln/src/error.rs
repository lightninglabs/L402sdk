//! Error types for the CLN backend adapters.

use l402_proto::ClientError;

/// Error type specific to the CLN backend.
#[derive(Debug, thiserror::Error)]
pub enum ClnError {
    /// HTTP or gRPC transport error.
    #[error("CLN transport error: {0}")]
    Transport(String),

    /// gRPC call returned an error status.
    #[cfg(feature = "grpc")]
    #[error("CLN gRPC error: {0}")]
    Rpc(#[from] tonic::Status),

    /// REST API returned an error.
    #[cfg(feature = "rest")]
    #[error("CLN REST API error ({status}): {body}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Response body.
        body: String,
    },

    /// Failed to read TLS certificate or key file.
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
impl From<tonic::transport::Error> for ClnError {
    fn from(err: tonic::transport::Error) -> Self {
        Self::Transport(err.to_string())
    }
}

#[cfg(feature = "rest")]
impl From<reqwest::Error> for ClnError {
    fn from(err: reqwest::Error) -> Self {
        Self::Transport(err.to_string())
    }
}

impl From<ClnError> for ClientError {
    fn from(err: ClnError) -> Self {
        match err {
            ClnError::Payment(reason) => ClientError::PaymentFailed { reason },
            other => ClientError::Backend {
                reason: other.to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = ClnError::Payment("no route found".to_string());
        assert_eq!(err.to_string(), "payment failed: no route found");

        let err = ClnError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert_eq!(err.to_string(), "IO error: file not found");
    }

    #[test]
    fn error_conversion_payment() {
        let cln_err = ClnError::Payment("timeout".to_string());
        let client_err: ClientError = cln_err.into();
        match client_err {
            ClientError::PaymentFailed { reason } => assert_eq!(reason, "timeout"),
            _ => panic!("expected PaymentFailed"),
        }
    }

    #[test]
    fn error_conversion_backend() {
        let cln_err = ClnError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "cert not found",
        ));
        let client_err: ClientError = cln_err.into();
        match client_err {
            ClientError::Backend { reason } => assert!(reason.contains("cert not found")),
            _ => panic!("expected Backend"),
        }
    }

    #[test]
    fn error_transport_display() {
        let err = ClnError::Transport("connection refused".to_string());
        assert_eq!(err.to_string(), "CLN transport error: connection refused");
    }

    #[cfg(feature = "rest")]
    #[test]
    fn error_api_display() {
        let err = ClnError::Api {
            status: 403,
            body: "forbidden".to_string(),
        };
        assert_eq!(err.to_string(), "CLN REST API error (403): forbidden");
    }

    #[cfg(feature = "rest")]
    #[test]
    fn error_deserialize_display() {
        let err = ClnError::Deserialize("invalid JSON".to_string());
        assert_eq!(err.to_string(), "deserialization error: invalid JSON");
    }
}
