//! CLN REST API backend implementation.
//!
//! Connects to a Core Lightning node using the CLN REST interface
//! and implements the [`LnBackend`] trait for invoice payments, balance
//! queries, and node info.
//!
//! This backend is suitable for WASM/browser environments where gRPC is
//! unavailable, and for setups where REST is simpler to configure.
//!
//! # Authentication
//!
//! Requests are authenticated via the `Rune` header containing a CLN rune
//! token. Runes are CLN's native bearer token system for API authorization.
//!
//! # Example
//!
//! ```rust,no_run
//! use l402_cln::ClnRestBackend;
//! use l402_proto::LnBackend;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let backend = ClnRestBackend::new(
//!     "https://localhost:3001",
//!     "rune_token_value...",
//! )?;
//!
//! let info = backend.get_info().await?;
//! println!("Connected to: {} ({})", info.alias, info.pubkey);
//! # Ok(())
//! # }
//! ```

use std::fmt;

use async_trait::async_trait;
use l402_proto::ClientError;
use l402_proto::{LnBackend, NodeInfo, PaymentResult};
use reqwest::Client as HttpClient;
use serde::Deserialize;

use crate::error::ClnError;

// ---------------------------------------------------------------------------
// ClnRestBackend
// ---------------------------------------------------------------------------

/// Core Lightning (CLN) Lightning backend via REST API.
///
/// Uses the CLN REST API to pay invoices, query balances, and retrieve
/// node information. Authenticated via rune token.
#[derive(Clone)]
pub struct ClnRestBackend {
    client: HttpClient,
    url: String,
    rune: String,
}

impl fmt::Debug for ClnRestBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClnRestBackend")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

impl ClnRestBackend {
    /// Create a new CLN REST backend using rune authentication.
    ///
    /// Runes are CLN's native bearer token system for API authorization.
    ///
    /// # Arguments
    ///
    /// * `url` - REST API endpoint (e.g. `https://localhost:3001`)
    /// * `rune` - Rune token string
    ///
    /// # Errors
    ///
    /// Returns [`ClnError::Transport`] if the HTTP client cannot be built.
    #[allow(clippy::result_large_err)]
    pub fn new(url: &str, rune: &str) -> Result<Self, ClnError> {
        let client = build_http_client()?;
        Ok(Self {
            client,
            url: url.trim_end_matches('/').to_string(),
            rune: rune.to_string(),
        })
    }

    /// Create a new CLN REST backend with a custom HTTP client.
    ///
    /// Useful for custom TLS configuration, proxies, or testing.
    pub fn with_client(url: &str, rune: &str, client: HttpClient) -> Self {
        Self {
            client,
            url: url.trim_end_matches('/').to_string(),
            rune: rune.to_string(),
        }
    }

    /// Create a CLN REST backend from environment variables.
    ///
    /// Reads from:
    /// - `CLN_REST_URL` (default: `https://localhost:3001`)
    /// - `CLN_RUNE` - rune token string
    ///
    /// # Errors
    ///
    /// Returns an error if `CLN_RUNE` is not set, or if the HTTP client
    /// cannot be built.
    ///
    /// Not available on WASM targets (use `new()` instead).
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(clippy::result_large_err)]
    pub fn from_env() -> Result<Self, ClnError> {
        let url =
            std::env::var("CLN_REST_URL").unwrap_or_else(|_| "https://localhost:3001".to_string());

        if let Ok(rune) = std::env::var("CLN_RUNE") {
            Self::new(&url, &rune)
        } else {
            Err(ClnError::Payment("CLN_RUNE is not set".to_string()))
        }
    }

    /// Attach authentication headers to a request builder.
    fn authenticate(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder.header("Rune", &self.rune)
    }
}

/// Build an HTTP client with appropriate defaults.
#[allow(clippy::result_large_err)]
fn build_http_client() -> Result<HttpClient, ClnError> {
    let builder = HttpClient::builder();
    #[cfg(not(target_arch = "wasm32"))]
    let builder = builder.danger_accept_invalid_certs(true);
    builder
        .build()
        .map_err(|e| ClnError::Transport(format!("failed to build HTTP client: {e}")))
}

// ---------------------------------------------------------------------------
// CLN REST API response types
// ---------------------------------------------------------------------------

/// Response from `POST /v1/pay`.
#[derive(Debug, Deserialize)]
struct PayResponse {
    payment_preimage: Option<String>,
    payment_hash: Option<String>,
    #[serde(alias = "msatoshi")]
    amount_msat: Option<MsatValue>,
    #[serde(alias = "msatoshi_sent")]
    amount_sent_msat: Option<MsatValue>,
    status: Option<String>,
}

/// Millisatoshi values returned by CLN REST.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MsatValue {
    /// Raw numeric millisatoshis.
    Number(u64),
    /// String values such as `"1000msat"`.
    String(String),
    /// Object values such as `{ "msat": 1000 }`.
    Object {
        /// Millisatoshis.
        msat: u64,
    },
}

impl MsatValue {
    /// Convert the deserialized value into raw millisatoshis.
    fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Number(value) | Self::Object { msat: value } => Some(*value),
            Self::String(value) => value.strip_suffix("msat").unwrap_or(value).parse().ok(),
        }
    }
}

/// Response from `POST /v1/listfunds`.
#[derive(Debug, Deserialize)]
struct ListFundsResponse {
    #[serde(default)]
    channels: Vec<ListFundsChannel>,
}

/// Channel balance entries returned by `listfunds`.
#[derive(Debug, Deserialize)]
struct ListFundsChannel {
    our_amount_msat: Option<MsatValue>,
}

/// Response from `POST /v1/getinfo`.
#[derive(Debug, Deserialize)]
struct GetInfoResponse {
    id: Option<String>,
    alias: Option<String>,
    num_active_channels: Option<u32>,
}

// ---------------------------------------------------------------------------
// LnBackend implementation
// ---------------------------------------------------------------------------

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl LnBackend for ClnRestBackend {
    async fn pay_invoice(
        &self,
        bolt11: &str,
        max_fee_sats: u64,
    ) -> Result<PaymentResult, ClientError> {
        let url = format!("{}/v1/pay", self.url);

        let body = serde_json::json!({
            "bolt11": bolt11,
            "maxfee": format!("{}msat", max_fee_sats.saturating_mul(1000)),
            "retry_for": 60,
        });

        let request = self.authenticate(self.client.post(&url)).json(&body);

        let response = request.send().await.map_err(ClnError::from)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ClnError::Api { status, body }.into());
        }

        let data: PayResponse = response
            .json()
            .await
            .map_err(|e| ClnError::Deserialize(format!("failed to parse payment response: {e}")))?;

        let status = data.status.as_deref().unwrap_or("unknown");
        if status != "complete" {
            return Err(ClnError::Payment(format!("payment status: {status}")).into());
        }

        let preimage = data.payment_preimage.ok_or_else(|| {
            ClnError::Payment("payment complete but returned empty preimage".to_string())
        })?;
        let payment_hash = data.payment_hash.ok_or_else(|| {
            ClnError::Payment("payment complete but returned empty hash".to_string())
        })?;

        let amount_msat = data
            .amount_msat
            .as_ref()
            .and_then(MsatValue::as_u64)
            .unwrap_or(0);
        let sent_msat = data
            .amount_sent_msat
            .as_ref()
            .and_then(MsatValue::as_u64)
            .unwrap_or(0);
        let fee_msat = sent_msat.saturating_sub(amount_msat);

        Ok(PaymentResult {
            preimage,
            payment_hash,
            amount_sats: amount_msat / 1000,
            fee_sats: fee_msat / 1000,
        })
    }

    async fn get_balance(&self) -> Result<u64, ClientError> {
        let url = format!("{}/v1/listfunds", self.url);

        let request = self
            .authenticate(self.client.post(&url))
            .json(&serde_json::json!({}));

        let response = request.send().await.map_err(ClnError::from)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ClnError::Api { status, body }.into());
        }

        let data: ListFundsResponse = response
            .json()
            .await
            .map_err(|e| ClnError::Deserialize(format!("failed to parse balance response: {e}")))?;

        let balance_msat: u64 = data
            .channels
            .iter()
            .map(|channel| {
                channel
                    .our_amount_msat
                    .as_ref()
                    .and_then(MsatValue::as_u64)
                    .unwrap_or(0)
            })
            .sum();

        Ok(balance_msat / 1000)
    }

    async fn get_info(&self) -> Result<NodeInfo, ClientError> {
        let url = format!("{}/v1/getinfo", self.url);

        let request = self
            .authenticate(self.client.post(&url))
            .json(&serde_json::json!({}));

        let response = request.send().await.map_err(ClnError::from)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ClnError::Api { status, body }.into());
        }

        let data: GetInfoResponse = response
            .json()
            .await
            .map_err(|e| ClnError::Deserialize(format!("failed to parse getinfo response: {e}")))?;

        Ok(NodeInfo {
            pubkey: data.id.unwrap_or_default(),
            alias: data.alias.unwrap_or_default(),
            num_active_channels: data.num_active_channels.unwrap_or(0),
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pay_response_complete() {
        let json = r#"{
            "payment_preimage": "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
            "payment_hash": "fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321",
            "amount_msat": "100000msat",
            "amount_sent_msat": "100500msat",
            "status": "complete"
        }"#;
        let resp: PayResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status.as_deref(), Some("complete"));
        assert_eq!(
            resp.amount_msat.as_ref().and_then(MsatValue::as_u64),
            Some(100_000)
        );
        assert_eq!(
            resp.amount_sent_msat.as_ref().and_then(MsatValue::as_u64),
            Some(100_500)
        );
        assert!(resp.payment_preimage.is_some());
        assert!(resp.payment_hash.is_some());
    }

    #[test]
    fn pay_response_failed() {
        let json = r#"{
            "status": "failed"
        }"#;
        let resp: PayResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status.as_deref(), Some("failed"));
        assert!(resp.payment_preimage.is_none());
    }

    #[test]
    fn pay_response_missing_amounts() {
        let json = r#"{
            "payment_preimage": "abc123",
            "payment_hash": "def456",
            "status": "complete"
        }"#;
        let resp: PayResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            resp.amount_msat
                .as_ref()
                .and_then(MsatValue::as_u64)
                .unwrap_or(0),
            0
        );
        assert_eq!(
            resp.amount_sent_msat
                .as_ref()
                .and_then(MsatValue::as_u64)
                .unwrap_or(0),
            0
        );
    }

    #[test]
    fn pay_response_legacy_amount_fields() {
        let json = r#"{
            "payment_preimage": "abc123",
            "payment_hash": "def456",
            "msatoshi": 500000,
            "msatoshi_sent": 500100,
            "status": "complete"
        }"#;
        let resp: PayResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            resp.amount_msat.as_ref().and_then(MsatValue::as_u64),
            Some(500_000)
        );
        assert_eq!(
            resp.amount_sent_msat.as_ref().and_then(MsatValue::as_u64),
            Some(500_100)
        );
    }

    #[test]
    fn msat_value_parses_multiple_shapes() {
        let number: MsatValue = serde_json::from_str("1000").unwrap();
        let string: MsatValue = serde_json::from_str(r#""2500msat""#).unwrap();
        let object: MsatValue = serde_json::from_str(r#"{"msat": 7500}"#).unwrap();

        assert_eq!(number.as_u64(), Some(1000));
        assert_eq!(string.as_u64(), Some(2500));
        assert_eq!(object.as_u64(), Some(7500));
    }

    #[test]
    fn list_funds_response_sums_channel_amounts() {
        let json = r#"{
            "channels": [
                {"our_amount_msat": "4000000msat"},
                {"our_amount_msat": {"msat": 1000000}},
                {"our_amount_msat": 500000}
            ]
        }"#;
        let resp: ListFundsResponse = serde_json::from_str(json).unwrap();
        let total: u64 = resp
            .channels
            .iter()
            .map(|channel| {
                channel
                    .our_amount_msat
                    .as_ref()
                    .and_then(MsatValue::as_u64)
                    .unwrap_or(0)
            })
            .sum();
        assert_eq!(total, 5_500_000);
    }

    #[test]
    fn get_info_response() {
        let json = r#"{
            "id": "02abc123def456789012345678901234567890123456789012345678901234567890",
            "alias": "my-cln-node",
            "num_active_channels": 5
        }"#;
        let resp: GetInfoResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            resp.id.unwrap(),
            "02abc123def456789012345678901234567890123456789012345678901234567890"
        );
        assert_eq!(resp.alias.unwrap(), "my-cln-node");
        assert_eq!(resp.num_active_channels.unwrap(), 5);
    }

    #[test]
    fn get_info_response_partial() {
        let json = r#"{"id": "02abc"}"#;
        let resp: GetInfoResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id.unwrap(), "02abc");
        assert!(resp.alias.is_none());
        assert!(resp.num_active_channels.is_none());
    }

    #[test]
    fn cln_rest_backend_debug() {
        let backend = ClnRestBackend::new("https://localhost:3001", "test_rune").unwrap();
        let debug = format!("{backend:?}");
        assert!(debug.contains("ClnRestBackend"));
        assert!(debug.contains("localhost:3001"));
        // Rune value should not leak in debug output.
        assert!(!debug.contains("test_rune"));
    }

    #[test]
    fn cln_rest_backend_trims_url() {
        let backend = ClnRestBackend::new("https://localhost:3001///", "test_rune").unwrap();
        assert_eq!(backend.url, "https://localhost:3001");
    }

    #[test]
    fn cln_rest_backend_clone() {
        let backend = ClnRestBackend::new("https://localhost:3001", "test_rune").unwrap();
        let cloned = backend.clone();
        assert_eq!(cloned.url, backend.url);
    }

    #[test]
    fn cln_rest_backend_with_client() {
        let client = HttpClient::new();
        let backend = ClnRestBackend::with_client("https://localhost:3001", "test_rune", client);
        assert_eq!(backend.url, "https://localhost:3001");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn from_env_missing_auth() {
        // SAFETY: test runs single-threaded; set_var is safe here.
        unsafe {
            std::env::remove_var("CLN_RUNE");
        }
        let result = ClnRestBackend::from_env();
        assert!(result.is_err());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn from_env_rune() {
        // SAFETY: test runs single-threaded; set_var is safe here.
        unsafe {
            std::env::set_var("CLN_RUNE", "test_rune_value");
            std::env::set_var("CLN_REST_URL", "https://mynode:3001");
        }
        let backend = ClnRestBackend::from_env().unwrap();
        assert_eq!(backend.url, "https://mynode:3001");
        // Clean up
        unsafe {
            std::env::remove_var("CLN_RUNE");
            std::env::remove_var("CLN_REST_URL");
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn from_env_default_url() {
        // SAFETY: test runs single-threaded; set_var is safe here.
        unsafe {
            std::env::remove_var("CLN_REST_URL");
            std::env::set_var("CLN_RUNE", "test_rune_value");
        }
        let backend = ClnRestBackend::from_env().unwrap();
        assert_eq!(backend.url, "https://localhost:3001");
        // Clean up
        unsafe {
            std::env::remove_var("CLN_RUNE");
        }
    }

    #[test]
    fn fee_calculation() {
        // Verify fee = sent - amount
        let amount_msat: u64 = 100_000;
        let sent_msat: u64 = 100_500;
        let fee_msat = sent_msat.saturating_sub(amount_msat);
        assert_eq!(fee_msat, 500);
        assert_eq!(fee_msat / 1000, 0); // Sub-sat fee rounds down
    }

    #[test]
    fn fee_calculation_zero_fee() {
        let amount_msat: u64 = 100_000;
        let sent_msat: u64 = 100_000;
        let fee_msat = sent_msat.saturating_sub(amount_msat);
        assert_eq!(fee_msat, 0);
    }
}
