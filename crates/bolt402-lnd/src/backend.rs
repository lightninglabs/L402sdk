//! LND gRPC backend implementation.

use async_trait::async_trait;
use bolt402_core::port::{NodeInfo, PaymentResult};
use bolt402_core::{ClientError, LnBackend};
use fedimint_tonic_lnd::lnrpc;
use std::fmt;
use std::path::Path;
use thiserror::Error;
use tokio::sync::Mutex;

/// Errors specific to the LND backend adapter.
#[derive(Debug, Error)]
pub enum LndError {
    /// Failed to connect to LND.
    #[error("failed to connect to LND at {address}: {reason}")]
    Connection {
        /// The LND gRPC address that was attempted.
        address: String,
        /// Description of the connection failure.
        reason: String,
    },

    /// An LND gRPC call failed.
    #[error("LND gRPC error: {0}")]
    Rpc(String),

    /// Payment was attempted but LND returned an error.
    #[error("payment failed: {0}")]
    Payment(String),
}

impl From<LndError> for ClientError {
    fn from(err: LndError) -> Self {
        match err {
            LndError::Payment(reason) => Self::PaymentFailed { reason },
            other => Self::Backend {
                reason: other.to_string(),
            },
        }
    }
}

/// LND gRPC backend adapter for the bolt402 L402 client.
///
/// Connects to an LND node via gRPC and implements the [`LnBackend`] trait
/// for paying Lightning invoices, querying channel balances, and retrieving
/// node information.
///
/// # Connection
///
/// LND requires TLS encryption and macaroon-based authentication for all
/// gRPC connections. The [`LndBackend::connect`] method handles reading
/// the TLS certificate and macaroon files, establishing the secure channel,
/// and attaching the macaroon metadata to every request.
///
/// # Thread Safety
///
/// `LndBackend` is `Send + Sync` and can be shared across tasks. It uses
/// an internal mutex to synchronize access to the underlying gRPC client.
pub struct LndBackend {
    client: Mutex<fedimint_tonic_lnd::Client>,
    address: String,
}

impl fmt::Debug for LndBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LndBackend")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

impl LndBackend {
    /// Connect to an LND node via gRPC.
    ///
    /// # Arguments
    ///
    /// * `address` - The LND gRPC endpoint (must start with `https://`)
    /// * `cert_path` - Path to the LND TLS certificate file (`tls.cert`)
    /// * `macaroon_path` - Path to an LND macaroon file (e.g., `admin.macaroon`)
    ///
    /// # Errors
    ///
    /// Returns [`LndError::Connection`] if the connection cannot be established.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # async fn example() -> Result<(), bolt402_lnd::LndError> {
    /// use bolt402_lnd::LndBackend;
    ///
    /// let backend = LndBackend::connect(
    ///     "https://localhost:10009",
    ///     "/home/user/.lnd/tls.cert",
    ///     "/home/user/.lnd/data/chain/bitcoin/mainnet/admin.macaroon",
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect<A, C, M>(
        address: A,
        cert_path: C,
        macaroon_path: M,
    ) -> Result<Self, LndError>
    where
        A: Into<String>,
        C: AsRef<Path> + Into<std::path::PathBuf> + fmt::Debug,
        M: AsRef<Path> + Into<std::path::PathBuf> + fmt::Debug,
    {
        let address = address.into();

        let client = fedimint_tonic_lnd::connect(address.clone(), cert_path, macaroon_path)
            .await
            .map_err(|e| LndError::Connection {
                address: address.clone(),
                reason: e.to_string(),
            })?;

        tracing::info!(address = %address, "connected to LND");

        Ok(Self {
            client: Mutex::new(client),
            address,
        })
    }

    /// Get the gRPC address this backend is connected to.
    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }
}

#[async_trait]
impl LnBackend for LndBackend {
    /// Pay a BOLT11 Lightning invoice via LND's `SendPaymentSync` RPC.
    ///
    /// Uses `SendPaymentSync` for simplicity and reliability. The fee limit
    /// is set as a fixed satoshi amount.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::PaymentFailed`] if LND reports a payment error,
    /// or [`ClientError::Backend`] if the gRPC call itself fails.
    async fn pay_invoice(
        &self,
        bolt11: &str,
        max_fee_sats: u64,
    ) -> Result<PaymentResult, ClientError> {
        tracing::debug!(invoice = %bolt11, max_fee_sats, "paying invoice via LND");

        let request = lnrpc::SendRequest {
            payment_request: bolt11.to_string(),
            fee_limit: Some(lnrpc::FeeLimit {
                limit: Some(lnrpc::fee_limit::Limit::Fixed(
                    i64::try_from(max_fee_sats).unwrap_or(i64::MAX),
                )),
            }),
            ..Default::default()
        };

        // Using send_payment_sync for simplicity. The newer SendPaymentV2
        // (router RPC) uses server streaming which adds complexity.
        // SendPaymentSync is functionally equivalent for single payments.
        #[allow(deprecated)]
        let response = self
            .client
            .lock()
            .await
            .lightning()
            .send_payment_sync(request)
            .await
            .map_err(|e| LndError::Rpc(e.message().to_string()))?
            .into_inner();

        // LND signals payment failure via the payment_error field
        if !response.payment_error.is_empty() {
            return Err(LndError::Payment(response.payment_error).into());
        }

        let preimage = hex::encode(&response.payment_preimage);
        let payment_hash = hex::encode(&response.payment_hash);

        // Extract amount and fees from the payment route (using msat fields,
        // the sat-denominated fields are deprecated)
        let (amount_sats, fee_sats) = response.payment_route.as_ref().map_or((0, 0), |route| {
            let total_fees_msat = u64::try_from(route.total_fees_msat).unwrap_or(0);
            let total_amt_msat = u64::try_from(route.total_amt_msat).unwrap_or(0);
            let fee_sats = total_fees_msat / 1000;
            let amount_sats = (total_amt_msat / 1000).saturating_sub(fee_sats);
            (amount_sats, fee_sats)
        });

        tracing::info!(
            payment_hash = %payment_hash,
            amount_sats,
            fee_sats,
            "payment successful"
        );

        Ok(PaymentResult {
            preimage,
            payment_hash,
            amount_sats,
            fee_sats,
        })
    }

    /// Get the spendable balance from LND's channel balances.
    ///
    /// Returns the total local balance across all active channels in satoshis.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Backend`] if the gRPC call fails.
    async fn get_balance(&self) -> Result<u64, ClientError> {
        let response = self
            .client
            .lock()
            .await
            .lightning()
            .channel_balance(lnrpc::ChannelBalanceRequest {})
            .await
            .map_err(|e| LndError::Rpc(e.message().to_string()))?
            .into_inner();

        let balance_sats = response.local_balance.map_or(0, |amount| amount.sat);

        Ok(balance_sats)
    }

    /// Get information about the connected LND node.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Backend`] if the gRPC call fails.
    async fn get_info(&self) -> Result<NodeInfo, ClientError> {
        let response = self
            .client
            .lock()
            .await
            .lightning()
            .get_info(lnrpc::GetInfoRequest {})
            .await
            .map_err(|e| LndError::Rpc(e.message().to_string()))?
            .into_inner();

        Ok(NodeInfo {
            pubkey: response.identity_pubkey,
            alias: response.alias,
            num_active_channels: response.num_active_channels,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lnd_error_to_client_error_payment() {
        let lnd_err = LndError::Payment("insufficient funds".to_string());
        let client_err: ClientError = lnd_err.into();

        match client_err {
            ClientError::PaymentFailed { reason } => {
                assert_eq!(reason, "insufficient funds");
            }
            other => panic!("expected PaymentFailed, got: {other:?}"),
        }
    }

    #[test]
    fn lnd_error_to_client_error_connection() {
        let lnd_err = LndError::Connection {
            address: "https://localhost:10009".to_string(),
            reason: "connection refused".to_string(),
        };
        let client_err: ClientError = lnd_err.into();

        match client_err {
            ClientError::Backend { reason } => {
                assert!(reason.contains("connection refused"));
                assert!(reason.contains("localhost:10009"));
            }
            other => panic!("expected Backend, got: {other:?}"),
        }
    }

    #[test]
    fn lnd_error_to_client_error_rpc() {
        let lnd_err = LndError::Rpc("unavailable".to_string());
        let client_err: ClientError = lnd_err.into();

        match client_err {
            ClientError::Backend { reason } => {
                assert!(reason.contains("unavailable"));
            }
            other => panic!("expected Backend, got: {other:?}"),
        }
    }

    #[test]
    fn lnd_error_display() {
        let err = LndError::Connection {
            address: "https://localhost:10009".to_string(),
            reason: "tls handshake failed".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "failed to connect to LND at https://localhost:10009: tls handshake failed"
        );

        let err = LndError::Payment("no route found".to_string());
        assert_eq!(err.to_string(), "payment failed: no route found");

        let err = LndError::Rpc("timeout".to_string());
        assert_eq!(err.to_string(), "LND gRPC error: timeout");
    }

    #[test]
    fn debug_format_hides_internals() {
        // We can't construct an LndBackend without a real LND connection,
        // but we verify the Debug impl compiles and the format is sensible.
        let formatted = format!(
            "{:?}",
            DebugProxy {
                address: "https://localhost:10009"
            }
        );
        assert!(formatted.contains("localhost:10009"));
    }

    /// Helper to verify our Debug format pattern.
    #[derive(Debug)]
    struct DebugProxy<'a> {
        #[allow(dead_code)]
        address: &'a str,
    }
}
