//! Error types for the NWC backend.

use l402_proto::ClientError;

/// Errors specific to the NWC (Nostr Wallet Connect) backend.
#[derive(Debug, thiserror::Error)]
pub enum NwcError {
    /// Invalid NWC connection URI.
    #[error("invalid NWC URI: {0}")]
    InvalidUri(String),

    /// The NWC client returned an error.
    #[error("NWC error: {0}")]
    Nwc(String),

    /// Payment failed with a specific reason.
    #[error("payment failed: {0}")]
    Payment(String),
}

impl From<nwc::Error> for NwcError {
    fn from(err: nwc::Error) -> Self {
        Self::Nwc(err.to_string())
    }
}

impl From<nostr::nips::nip47::Error> for NwcError {
    fn from(err: nostr::nips::nip47::Error) -> Self {
        Self::InvalidUri(err.to_string())
    }
}

impl From<NwcError> for ClientError {
    fn from(err: NwcError) -> Self {
        match err {
            NwcError::Payment(reason) => ClientError::PaymentFailed { reason },
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
        let err = NwcError::Payment("insufficient balance".to_string());
        assert_eq!(err.to_string(), "payment failed: insufficient balance");

        let err = NwcError::InvalidUri("missing relay parameter".to_string());
        assert_eq!(err.to_string(), "invalid NWC URI: missing relay parameter");

        let err = NwcError::Nwc("connection timeout".to_string());
        assert_eq!(err.to_string(), "NWC error: connection timeout");
    }

    #[test]
    fn error_conversion_payment() {
        let nwc_err = NwcError::Payment("timeout".to_string());
        let client_err: ClientError = nwc_err.into();
        match client_err {
            ClientError::PaymentFailed { reason } => assert_eq!(reason, "timeout"),
            _ => panic!("expected PaymentFailed"),
        }
    }

    #[test]
    fn error_conversion_backend() {
        let nwc_err = NwcError::InvalidUri("bad uri".to_string());
        let client_err: ClientError = nwc_err.into();
        match client_err {
            ClientError::Backend { reason } => {
                assert!(reason.contains("bad uri"));
            }
            _ => panic!("expected Backend"),
        }
    }
}
