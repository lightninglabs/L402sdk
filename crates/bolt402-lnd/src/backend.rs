//! LND backend implementation using vendored gRPC protos.

use std::fmt;

use async_trait::async_trait;
use bolt402_core::ClientError;
use bolt402_core::port::{LnBackend, NodeInfo, PaymentResult};
use tonic::Request;
use tonic::transport::{Certificate, Channel, ClientTlsConfig};

use crate::lnrpc;
use crate::lnrpc::lightning_client::LightningClient;
use crate::routerrpc;
use crate::routerrpc::router_client::RouterClient;

/// Error type specific to the LND backend.
#[derive(Debug, thiserror::Error)]
pub enum LndError {
    /// gRPC connection or transport error.
    #[error("LND transport error: {0}")]
    Transport(#[from] tonic::transport::Error),

    /// gRPC call returned an error status.
    #[error("LND gRPC error: {0}")]
    Rpc(#[from] tonic::Status),

    /// Failed to read TLS certificate or macaroon file.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Payment failed with a specific reason.
    #[error("payment failed: {0}")]
    Payment(String),
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

/// Type alias for an intercepted gRPC channel with macaroon auth.
type LndChannel = tonic::service::interceptor::InterceptedService<Channel, MacaroonInterceptor>;

/// LND Lightning backend via gRPC.
///
/// Connects to an LND node using TLS and macaroon authentication,
/// then implements the [`LnBackend`] trait for invoice payments.
/// Uses `SendPaymentV2` from the router RPC for payment execution.
pub struct LndBackend {
    client: LightningClient<LndChannel>,
    router: RouterClient<LndChannel>,
    address: String,
}

impl fmt::Debug for LndBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LndBackend")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

/// Interceptor that injects the macaroon into every gRPC request.
#[derive(Clone)]
struct MacaroonInterceptor {
    macaroon: String,
}

impl tonic::service::Interceptor for MacaroonInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, tonic::Status> {
        request.metadata_mut().insert(
            "macaroon",
            self.macaroon
                .parse()
                .map_err(|_| tonic::Status::internal("invalid macaroon metadata value"))?,
        );
        Ok(request)
    }
}

impl LndBackend {
    /// Connect to an LND node.
    ///
    /// # Arguments
    ///
    /// * `address` - gRPC endpoint (e.g. `https://localhost:10009`)
    /// * `tls_cert_path` - Path to LND's `tls.cert` file
    /// * `macaroon_path` - Path to an admin or invoice macaroon file
    pub async fn connect(
        address: &str,
        tls_cert_path: &str,
        macaroon_path: &str,
    ) -> Result<Self, LndError> {
        // Read TLS certificate
        let tls_cert = tokio::fs::read(tls_cert_path).await?;
        let tls_config = ClientTlsConfig::new()
            .ca_certificate(Certificate::from_pem(tls_cert))
            .domain_name("localhost");

        // Read macaroon and hex-encode it
        let macaroon_bytes = tokio::fs::read(macaroon_path).await?;
        let macaroon_hex = hex::encode(&macaroon_bytes);

        // Build the gRPC channel
        let channel = Channel::from_shared(address.to_string())
            .map_err(|e| LndError::Payment(format!("invalid address: {e}")))?
            .tls_config(tls_config)?
            .connect()
            .await?;

        let interceptor = MacaroonInterceptor {
            macaroon: macaroon_hex,
        };
        let client = LightningClient::with_interceptor(channel.clone(), interceptor.clone());
        let router = RouterClient::with_interceptor(channel, interceptor);

        Ok(Self {
            client,
            router,
            address: address.to_string(),
        })
    }

    /// Connect using environment variables.
    ///
    /// Reads from:
    /// - `LND_GRPC_HOST` (default: `https://localhost:10009`)
    /// - `LND_TLS_CERT_PATH` (default: `~/.lnd/tls.cert`)
    /// - `LND_MACAROON_PATH` (default: `~/.lnd/data/chain/bitcoin/mainnet/admin.macaroon`)
    pub async fn from_env() -> Result<Self, LndError> {
        let host = std::env::var("LND_GRPC_HOST")
            .unwrap_or_else(|_| "https://localhost:10009".to_string());
        let cert_path =
            std::env::var("LND_TLS_CERT_PATH").unwrap_or_else(|_| expand_home("~/.lnd/tls.cert"));
        let macaroon_path = std::env::var("LND_MACAROON_PATH")
            .unwrap_or_else(|_| expand_home("~/.lnd/data/chain/bitcoin/mainnet/admin.macaroon"));

        Self::connect(&host, &cert_path, &macaroon_path).await
    }
}

#[async_trait]
impl LnBackend for LndBackend {
    async fn pay_invoice(
        &self,
        bolt11: &str,
        max_fee_sats: u64,
    ) -> Result<PaymentResult, ClientError> {
        let request = routerrpc::SendPaymentRequest {
            payment_request: bolt11.to_string(),
            fee_limit_sat: i64::try_from(max_fee_sats).unwrap_or(i64::MAX),
            timeout_seconds: 60,
            no_inflight_updates: true,
            ..Default::default()
        };

        // SendPaymentV2 returns a stream; with no_inflight_updates=true
        // we get a single message when the payment completes.
        let stream = self
            .router
            .clone()
            .send_payment_v2(request)
            .await
            .map_err(LndError::from)?;

        let payment = stream
            .into_inner()
            .message()
            .await
            .map_err(LndError::from)?
            .ok_or_else(|| LndError::Payment("no payment response received".to_string()))?;

        match payment.status() {
            lnrpc::payment::PaymentStatus::Succeeded => {
                let preimage = payment.payment_preimage;
                let payment_hash = payment.payment_hash;

                // Extract amounts from the payment
                let value_msat = u64::try_from(payment.value_msat).unwrap_or(0);
                let fee_msat = u64::try_from(payment.fee_msat).unwrap_or(0);

                Ok(PaymentResult {
                    preimage,
                    payment_hash,
                    amount_sats: value_msat / 1000,
                    fee_sats: fee_msat / 1000,
                })
            }
            lnrpc::payment::PaymentStatus::Failed => {
                Err(LndError::Payment(format!("{:?}", payment.failure_reason())).into())
            }
            status => Err(LndError::Payment(format!("unexpected status: {status:?}")).into()),
        }
    }

    async fn get_balance(&self) -> Result<u64, ClientError> {
        let response = self
            .client
            .clone()
            .channel_balance(lnrpc::ChannelBalanceRequest {})
            .await
            .map_err(LndError::from)?
            .into_inner();

        let balance = response.local_balance.map_or(0, |b| b.sat);
        Ok(balance)
    }

    async fn get_info(&self) -> Result<NodeInfo, ClientError> {
        let response = self
            .client
            .clone()
            .get_info(lnrpc::GetInfoRequest {})
            .await
            .map_err(LndError::from)?
            .into_inner();

        Ok(NodeInfo {
            pubkey: response.identity_pubkey,
            alias: response.alias,
            num_active_channels: response.num_active_channels,
        })
    }
}

/// Expand `~` to the home directory.
fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = LndError::Payment("no route found".to_string());
        assert_eq!(err.to_string(), "payment failed: no route found");

        let err = LndError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert_eq!(err.to_string(), "IO error: file not found");
    }

    #[test]
    fn error_conversion() {
        let lnd_err = LndError::Payment("timeout".to_string());
        let client_err: ClientError = lnd_err.into();
        match client_err {
            ClientError::PaymentFailed { reason } => assert_eq!(reason, "timeout"),
            _ => panic!("expected PaymentFailed"),
        }
    }

    #[test]
    fn expand_home_tilde() {
        // SAFETY: test runs single-threaded; set_var is safe here.
        unsafe { std::env::set_var("HOME", "/home/test") };
        assert_eq!(expand_home("~/.lnd/tls.cert"), "/home/test/.lnd/tls.cert");
        assert_eq!(expand_home("/absolute/path"), "/absolute/path");
    }
}
