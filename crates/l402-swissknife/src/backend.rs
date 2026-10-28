//! SwissKnife backend implementation using the REST API.

use std::fmt;

use async_trait::async_trait;
use l402_proto::ClientError;
use l402_proto::port::{LnBackend, NodeInfo, PaymentResult};
use reqwest::Client as HttpClient;
use reqwest::header::HeaderValue;

use crate::types::{
    BalanceResponse, ErrorResponse, PaymentStatus, SendPaymentRequest, WalletResponse,
};

/// Error type specific to the SwissKnife backend.
#[derive(Debug, thiserror::Error)]
pub enum SwissKnifeError {
    /// HTTP transport or request error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// SwissKnife API returned an error response.
    #[error("API error (HTTP {status}): {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Error message from the API.
        message: String,
    },

    /// Authentication failed (HTTP 401 or 403).
    #[error("authentication failed: {0}")]
    Auth(String),

    /// Payment failed.
    #[error("payment failed: {0}")]
    Payment(String),

    /// Missing required configuration.
    #[error("missing configuration: {0}")]
    Config(String),
}

impl From<SwissKnifeError> for ClientError {
    fn from(err: SwissKnifeError) -> Self {
        match err {
            SwissKnifeError::Payment(reason) => ClientError::PaymentFailed { reason },
            SwissKnifeError::Auth(reason) => ClientError::Backend {
                reason: format!("authentication failed: {reason}"),
            },
            other => ClientError::Backend {
                reason: other.to_string(),
            },
        }
    }
}

/// SwissKnife Lightning backend via REST API.
///
/// Connects to a Numeraire SwissKnife instance using API key authentication
/// and implements the [`LnBackend`] trait for invoice payments, balance queries,
/// and wallet information.
///
/// # Authentication
///
/// Uses API key authentication via the `Api-Key: <key>` header.
/// API keys can be created from the SwissKnife dashboard and require
/// `read:transaction` and `write:transaction` permissions.
///
/// # Example
///
/// ```rust,no_run
/// use l402_swissknife::SwissKnifeBackend;
/// use l402_proto::LnBackend;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = SwissKnifeBackend::new(
///     "https://api.numeraire.tech",
///     "your-api-key",
/// );
///
/// let balance = backend.get_balance().await?;
/// println!("Balance: {} sats", balance);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct SwissKnifeBackend {
    client: HttpClient,
    base_url: String,
    api_key: String,
    wallet_id: Option<String>,
}

impl fmt::Debug for SwissKnifeBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SwissKnifeBackend")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl SwissKnifeBackend {
    /// Create a new `SwissKnifeBackend` connected to the given instance.
    ///
    /// # Arguments
    ///
    /// * `base_url` - Base URL of the SwissKnife instance (e.g. `https://api.numeraire.tech`)
    /// * `api_key` - API key with `read:transaction` and `write:transaction` permissions
    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self {
            client: HttpClient::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            wallet_id: None,
        }
    }

    /// Select the wallet used by SwissKnife 0.3 and later.
    ///
    /// The wallet must belong to the API key's account. Older servers can keep
    /// using [`Self::new`] without a selector. An invalid UUID is rejected before
    /// constructing any request path.
    pub fn with_wallet(mut self, wallet_id: &str) -> Result<Self, SwissKnifeError> {
        if wallet_id.len() != 36
            || !wallet_id.bytes().enumerate().all(|(i, c)| {
                if [8, 13, 18, 23].contains(&i) {
                    c == b'-'
                } else {
                    c.is_ascii_hexdigit()
                }
            })
        {
            return Err(SwissKnifeError::Config(
                "wallet ID must be a UUID".to_string(),
            ));
        }
        self.wallet_id = Some(wallet_id.to_string());
        Ok(self)
    }

    fn wallet_path(&self, suffix: &str) -> String {
        match &self.wallet_id {
            Some(id) => format!("/v1/me/wallets/{id}{suffix}"),
            None => format!("/v1/me{suffix}"),
        }
    }

    /// Create a new `SwissKnifeBackend` using environment variables.
    ///
    /// Reads from:
    /// - `SWISSKNIFE_API_URL` (default: `https://api.numeraire.tech`)
    /// - `SWISSKNIFE_API_KEY` (required)
    /// - `SWISSKNIFE_WALLET_ID` (required for SwissKnife 0.3+)
    ///
    /// # Errors
    ///
    /// Returns [`SwissKnifeError::Config`] if `SWISSKNIFE_API_KEY` is not set.
    pub fn from_env() -> Result<Self, SwissKnifeError> {
        let base_url = std::env::var("SWISSKNIFE_API_URL")
            .unwrap_or_else(|_| "https://api.numeraire.tech".to_string());

        let api_key = std::env::var("SWISSKNIFE_API_KEY")
            .map_err(|_| SwissKnifeError::Config("SWISSKNIFE_API_KEY is not set".to_string()))?;

        let backend = Self::new(&base_url, &api_key);
        match std::env::var("SWISSKNIFE_WALLET_ID") {
            Ok(id) => backend.with_wallet(&id),
            Err(_) => Ok(backend),
        }
    }

    /// Build an authenticated request to the SwissKnife API.
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{path}", self.base_url);

        self.client.request(method, &url).header(
            "Api-Key",
            HeaderValue::from_str(&self.api_key)
                .expect("API key contains invalid header characters"),
        )
    }

    /// Parse an error response from the SwissKnife API.
    async fn parse_error(status: u16, response: reqwest::Response) -> SwissKnifeError {
        let message = match response.json::<ErrorResponse>().await {
            Ok(err) => err.reason.unwrap_or_else(|| format!("HTTP {status} error")),
            Err(_) => format!("HTTP {status} error"),
        };

        if status == 401 || status == 403 {
            SwissKnifeError::Auth(message)
        } else {
            SwissKnifeError::Api { status, message }
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl LnBackend for SwissKnifeBackend {
    async fn pay_invoice(
        &self,
        bolt11: &str,
        max_fee_sats: u64,
    ) -> Result<PaymentResult, ClientError> {
        let body = SendPaymentRequest {
            input: bolt11.to_string(),
        };

        let response = self
            .request(reqwest::Method::POST, &self.wallet_path("/payments"))
            .json(&body)
            .send()
            .await
            .map_err(SwissKnifeError::from)?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            return Err(Self::parse_error(status, response).await.into());
        }

        let payment: crate::types::PaymentResponse =
            response.json().await.map_err(SwissKnifeError::from)?;

        // Check payment status
        match payment.status {
            PaymentStatus::Settled => {}
            PaymentStatus::Failed => {
                let reason = payment
                    .error
                    .unwrap_or_else(|| "payment failed without error message".to_string());
                return Err(SwissKnifeError::Payment(reason).into());
            }
            PaymentStatus::Pending => {
                return Err(SwissKnifeError::Payment(
                    "payment is still pending (not yet settled)".to_string(),
                )
                .into());
            }
            PaymentStatus::Unknown => {
                return Err(SwissKnifeError::Payment(
                    "unexpected payment status from SwissKnife".to_string(),
                )
                .into());
            }
        }

        let (payment_hash, payment_preimage) = match payment.lightning {
            Some(proof) => (proof.payment_hash, proof.payment_preimage),
            None => (
                payment.payment_hash.unwrap_or_default(),
                payment.payment_preimage,
            ),
        };
        let preimage = payment_preimage.ok_or_else(|| {
            SwissKnifeError::Payment("settled payment missing preimage".to_string())
        })?;

        let amount_sats = payment.amount_msat / 1000;
        let fee_sats = payment.fee_msat.unwrap_or(0) / 1000;

        // Warn if actual fee exceeds the requested max (payment already settled,
        // so we can't undo it, but the caller should know).
        if fee_sats > max_fee_sats {
            tracing::warn!(
                fee_sats,
                max_fee_sats,
                "SwissKnife payment fee ({fee_sats} sats) exceeds requested max ({max_fee_sats} sats)"
            );
        }

        Ok(PaymentResult {
            preimage,
            payment_hash,
            amount_sats,
            fee_sats,
        })
    }

    async fn get_balance(&self) -> Result<u64, ClientError> {
        let response = self
            .request(reqwest::Method::GET, &self.wallet_path("/balance"))
            .send()
            .await
            .map_err(SwissKnifeError::from)?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            return Err(Self::parse_error(status, response).await.into());
        }

        let balance: BalanceResponse = response.json().await.map_err(SwissKnifeError::from)?;

        // Convert msat to sats, clamping negative balances to 0
        let sats = u64::try_from(balance.available_msat.max(0)).unwrap_or(0) / 1000;

        Ok(sats)
    }

    async fn get_info(&self) -> Result<NodeInfo, ClientError> {
        let response = self
            .request(reqwest::Method::GET, &self.wallet_path(""))
            .send()
            .await
            .map_err(SwissKnifeError::from)?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            return Err(Self::parse_error(status, response).await.into());
        }

        let wallet: WalletResponse = response.json().await.map_err(SwissKnifeError::from)?;

        Ok(NodeInfo {
            // SwissKnife wallets are custodial accounts, not nodes.
            // Use wallet ID as a unique identifier in place of a pubkey.
            pubkey: wallet.id,
            alias: if wallet.user_id.is_empty() {
                "SwissKnife Wallet".to_string()
            } else {
                wallet.user_id
            },
            // Not applicable for custodial wallets.
            num_active_channels: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallet_paths_require_explicit_valid_selector() {
        let backend = SwissKnifeBackend::new("https://example.com", "test");
        assert_eq!(backend.wallet_path("/balance"), "/v1/me/balance");
        let id = "12345678-1234-1234-1234-123456789abc";
        let selected = backend.with_wallet(id).unwrap();
        assert_eq!(
            selected.wallet_path("/payments"),
            format!("/v1/me/wallets/{id}/payments")
        );
        for invalid in ["", "../payments", "12345678-1234-1234-1234-123456789ab/"] {
            assert!(
                SwissKnifeBackend::new("https://example.com", "test")
                    .with_wallet(invalid)
                    .is_err()
            );
        }
    }

    #[test]
    fn error_display() {
        let err = SwissKnifeError::Payment("no route".to_string());
        assert_eq!(err.to_string(), "payment failed: no route");

        let err = SwissKnifeError::Auth("invalid key".to_string());
        assert_eq!(err.to_string(), "authentication failed: invalid key");

        let err = SwissKnifeError::Api {
            status: 500,
            message: "internal error".to_string(),
        };
        assert_eq!(err.to_string(), "API error (HTTP 500): internal error");

        let err = SwissKnifeError::Config("missing key".to_string());
        assert_eq!(err.to_string(), "missing configuration: missing key");
    }

    #[test]
    fn error_conversion_payment() {
        let sk_err = SwissKnifeError::Payment("timeout".to_string());
        let client_err: ClientError = sk_err.into();
        match client_err {
            ClientError::PaymentFailed { reason } => assert_eq!(reason, "timeout"),
            other => panic!("expected PaymentFailed, got {other:?}"),
        }
    }

    #[test]
    fn error_conversion_auth() {
        let sk_err = SwissKnifeError::Auth("bad token".to_string());
        let client_err: ClientError = sk_err.into();
        match client_err {
            ClientError::Backend { reason } => {
                assert!(reason.contains("authentication failed"));
                assert!(reason.contains("bad token"));
            }
            other => panic!("expected Backend, got {other:?}"),
        }
    }

    #[test]
    fn error_conversion_api() {
        let sk_err = SwissKnifeError::Api {
            status: 422,
            message: "unprocessable".to_string(),
        };
        let client_err: ClientError = sk_err.into();
        match client_err {
            ClientError::Backend { reason } => {
                assert!(reason.contains("422"));
                assert!(reason.contains("unprocessable"));
            }
            other => panic!("expected Backend, got {other:?}"),
        }
    }

    #[test]
    fn new_trims_trailing_slash() {
        let backend = SwissKnifeBackend::new("https://example.com/", "key");
        assert_eq!(backend.base_url, "https://example.com");
    }

    #[test]
    fn new_preserves_clean_url() {
        let backend = SwissKnifeBackend::new("https://example.com", "key");
        assert_eq!(backend.base_url, "https://example.com");
    }

    #[test]
    fn debug_does_not_leak_api_key() {
        let backend = SwissKnifeBackend::new("https://example.com", "secret-key-123");
        let debug = format!("{backend:?}");
        assert!(!debug.contains("secret-key-123"));
        assert!(debug.contains("https://example.com"));
    }

    /// Env-based tests are combined into a single test to avoid race conditions
    /// from parallel test execution mutating shared process environment.
    #[test]
    fn from_env_scenarios() {
        // Scenario 1: Missing API key → error
        unsafe {
            std::env::remove_var("SWISSKNIFE_API_KEY");
            std::env::remove_var("SWISSKNIFE_API_URL");
        }
        let result = SwissKnifeBackend::from_env();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("SWISSKNIFE_API_KEY"));

        // Scenario 2: API key set, default URL
        unsafe {
            std::env::set_var("SWISSKNIFE_API_KEY", "test-key");
            std::env::remove_var("SWISSKNIFE_API_URL");
        }
        let result = SwissKnifeBackend::from_env();
        assert!(result.is_ok());
        let backend = result.unwrap();
        assert_eq!(backend.base_url, "https://api.numeraire.tech");

        // Scenario 3: Custom URL
        unsafe {
            std::env::set_var("SWISSKNIFE_API_URL", "https://custom.host:3000");
        }
        let result = SwissKnifeBackend::from_env();
        assert!(result.is_ok());
        let backend = result.unwrap();
        assert_eq!(backend.base_url, "https://custom.host:3000");

        // Cleanup
        unsafe {
            std::env::remove_var("SWISSKNIFE_API_KEY");
            std::env::remove_var("SWISSKNIFE_API_URL");
        }
    }

    #[test]
    fn msat_to_sats_conversion() {
        // 100_000 msat = 100 sats
        assert_eq!(100_000u64 / 1000, 100);
        // 1_500 msat = 1 sat (truncated)
        assert_eq!(1_500u64 / 1000, 1);
        // 999 msat = 0 sats (truncated)
        assert_eq!(999u64 / 1000, 0);
    }

    #[test]
    fn negative_balance_clamped_to_zero() {
        let available_msat: i64 = -5000;
        let sats = u64::try_from(available_msat.max(0)).unwrap_or(0) / 1000;
        assert_eq!(sats, 0);
    }
}
