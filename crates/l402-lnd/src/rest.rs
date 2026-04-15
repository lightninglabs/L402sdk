//! LND REST API backend implementation.
//!
//! Connects to an LND node using its REST API (default port 8080) and
//! implements the [`LnBackend`] trait for invoice payments, balance queries,
//! and node info.
//!
//! This backend is suitable for WASM/browser environments where gRPC is
//! unavailable, and for setups where REST is simpler to configure.
//!
//! # Authentication
//!
//! Requests are authenticated via the `Grpc-Metadata-macaroon` header
//! containing a hex-encoded macaroon.
//!
//! # Example
//!
//! ```rust,no_run
//! use l402_lnd::LndRestBackend;
//! use l402_proto::LnBackend;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let backend = LndRestBackend::new(
//!     "https://localhost:8080",
//!     "0201036c6e640258030a...",  // hex-encoded macaroon
//! )?;
//!
//! let info = backend.get_info().await?;
//! println!("Connected to: {} ({})", info.alias, info.pubkey);
//! # Ok(())
//! # }
//! ```

use std::fmt;

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use l402_proto::ClientError;
use l402_proto::{LnBackend, NodeInfo, PaymentResult};
use reqwest::Client as HttpClient;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::LndError;

/// LND Lightning backend via REST API.
///
/// Uses LND's REST API (grpc-gateway) to pay invoices, query balances,
/// and retrieve node information. Authenticated via hex-encoded macaroon.
#[derive(Clone)]
pub struct LndRestBackend {
    client: HttpClient,
    url: String,
    macaroon: String,
}

impl fmt::Debug for LndRestBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LndRestBackend")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

impl LndRestBackend {
    /// Create a new LND REST backend.
    ///
    /// # Arguments
    ///
    /// * `url` - REST API endpoint (e.g. `https://localhost:8080`)
    /// * `macaroon` - Hex-encoded admin macaroon
    ///
    /// # Errors
    ///
    /// Returns [`LndError::Transport`] if the HTTP client cannot be built.
    #[allow(clippy::result_large_err)]
    pub fn new(url: &str, macaroon: &str) -> Result<Self, LndError> {
        let builder = HttpClient::builder();
        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder.danger_accept_invalid_certs(true);
        let client = builder
            .build()
            .map_err(|e| LndError::Transport(format!("failed to build HTTP client: {e}")))?;

        Ok(Self {
            client,
            url: url.trim_end_matches('/').to_string(),
            macaroon: macaroon.to_string(),
        })
    }

    /// Create a new LND REST backend with a custom HTTP client.
    ///
    /// Useful for custom TLS configuration, proxies, or testing.
    pub fn with_client(url: &str, macaroon: &str, client: HttpClient) -> Self {
        Self {
            client,
            url: url.trim_end_matches('/').to_string(),
            macaroon: macaroon.to_string(),
        }
    }

    /// Create an LND REST backend from environment variables.
    ///
    /// Reads from:
    /// - `LND_REST_URL` (default: `https://localhost:8080`)
    /// - `LND_MACAROON_HEX` - hex-encoded macaroon string (preferred)
    /// - `LND_MACAROON_PATH` - path to binary macaroon file (fallback, uses sync I/O)
    ///
    /// If both `LND_MACAROON_HEX` and `LND_MACAROON_PATH` are set,
    /// `LND_MACAROON_HEX` takes precedence.
    ///
    /// # Errors
    ///
    /// Returns an error if neither macaroon variable is set, or if the
    /// macaroon file cannot be read.
    ///
    /// Not available on WASM targets (use `new()` instead).
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(clippy::result_large_err)]
    pub fn from_env() -> Result<Self, LndError> {
        let url =
            std::env::var("LND_REST_URL").unwrap_or_else(|_| "https://localhost:8080".to_string());

        let macaroon = if let Ok(hex) = std::env::var("LND_MACAROON_HEX") {
            hex
        } else if let Ok(path) = std::env::var("LND_MACAROON_PATH") {
            let bytes = std::fs::read(&path)?;
            hex::encode(&bytes)
        } else {
            return Err(LndError::Payment(
                "neither LND_MACAROON_HEX nor LND_MACAROON_PATH is set".to_string(),
            ));
        };

        Self::new(&url, &macaroon)
    }
}

// ---------------------------------------------------------------------------
// LND REST API response types
// ---------------------------------------------------------------------------

/// Payment result from `/v2/router/send`.
///
/// Supports both `snake_case` (standard grpc-gateway) and `camelCase`
/// (some LND/wrapper variants) field names.
#[derive(Debug, Deserialize)]
struct SendPaymentResponse {
    result: Option<PaymentUpdate>,
    // Flattened form (when result wrapper is absent)
    #[serde(flatten)]
    flat: PaymentUpdate,
}

#[derive(Debug, Default, Deserialize)]
struct PaymentUpdate {
    status: Option<String>,
    payment_preimage: Option<String>,
    #[serde(rename = "paymentPreimage")]
    payment_preimage_camel: Option<String>,
    payment_hash: Option<String>,
    #[serde(rename = "paymentHash")]
    payment_hash_camel: Option<String>,
    value_sat: Option<String>,
    #[serde(rename = "valueSat")]
    value_sat_camel: Option<String>,
    value_msat: Option<String>,
    #[serde(rename = "valueMsat")]
    value_msat_camel: Option<String>,
    fee_sat: Option<String>,
    #[serde(rename = "feeSat")]
    fee_sat_camel: Option<String>,
    fee_msat: Option<String>,
    #[serde(rename = "feeMsat")]
    fee_msat_camel: Option<String>,
    failure_reason: Option<String>,
}

impl PaymentUpdate {
    fn preimage(&self) -> Option<&str> {
        self.payment_preimage
            .as_deref()
            .or(self.payment_preimage_camel.as_deref())
    }

    fn hash(&self) -> Option<&str> {
        self.payment_hash
            .as_deref()
            .or(self.payment_hash_camel.as_deref())
    }

    fn amount_sats(&self) -> u64 {
        // Prefer millisat values (more precise), fall back to sat values
        if let Some(msat) = self
            .value_msat
            .as_deref()
            .or(self.value_msat_camel.as_deref())
        {
            return msat.parse::<u64>().unwrap_or(0) / 1000;
        }
        self.value_sat
            .as_deref()
            .or(self.value_sat_camel.as_deref())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }

    fn fee_sats(&self) -> u64 {
        if let Some(msat) = self.fee_msat.as_deref().or(self.fee_msat_camel.as_deref()) {
            return msat.parse::<u64>().unwrap_or(0) / 1000;
        }
        self.fee_sat
            .as_deref()
            .or(self.fee_sat_camel.as_deref())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }

    fn is_succeeded(&self) -> bool {
        self.status.as_deref() == Some("SUCCEEDED")
    }

    fn is_failed(&self) -> bool {
        self.status.as_deref() == Some("FAILED")
    }
}

/// Response from `/v1/balance/channels`.
#[derive(Debug, Deserialize)]
struct ChannelBalanceResponse {
    local_balance: Option<BalanceAmount>,
}

#[derive(Debug, Deserialize)]
struct BalanceAmount {
    sat: Option<String>,
}

/// Response from `/v1/getinfo`.
#[derive(Debug, Deserialize)]
struct GetInfoResponse {
    identity_pubkey: Option<String>,
    alias: Option<String>,
    num_active_channels: Option<u32>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Normalize a bytes field from LND REST API to a hex string.
///
/// LND's grpc-gateway encodes protobuf `bytes` fields as base64 by default.
/// Some LND wrappers or proxy configurations return hex instead. This
/// function detects the encoding and normalizes to lowercase hex.
///
/// Detection heuristic:
/// - If the string is exactly 64 hex characters (32 bytes), treat as hex.
/// - Otherwise, decode as standard base64 and convert to hex.
#[allow(clippy::result_large_err)]
fn bytes_field_to_hex(raw: &str) -> Result<String, LndError> {
    if raw.is_empty() {
        return Err(LndError::Deserialize("empty bytes field".to_string()));
    }

    // Check if already hex (32 bytes = 64 hex chars)
    if raw.len() == 64 && raw.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Ok(raw.to_ascii_lowercase());
    }

    // Decode as base64
    let bytes = BASE64
        .decode(raw)
        .map_err(|e| LndError::Deserialize(format!("invalid base64 in bytes field: {e}")))?;

    Ok(hex::encode(&bytes))
}

/// Verify that SHA-256(preimage) equals the payment hash.
#[allow(clippy::result_large_err)]
fn verify_preimage(preimage_hex: &str, payment_hash_hex: &str) -> Result<(), LndError> {
    let preimage_bytes = hex::decode(preimage_hex)
        .map_err(|e| LndError::Deserialize(format!("invalid preimage hex: {e}")))?;

    let mut hasher = Sha256::new();
    hasher.update(&preimage_bytes);
    let computed = hex::encode(hasher.finalize());

    if computed != payment_hash_hex {
        return Err(LndError::Payment(format!(
            "preimage verification failed: SHA256(preimage) {computed} != payment_hash {payment_hash_hex}"
        )));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// LnBackend implementation
// ---------------------------------------------------------------------------

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl LnBackend for LndRestBackend {
    async fn pay_invoice(
        &self,
        bolt11: &str,
        max_fee_sats: u64,
    ) -> Result<PaymentResult, ClientError> {
        let url = format!("{}/v2/router/send", self.url);

        let body = serde_json::json!({
            "payment_request": bolt11,
            "fee_limit_sat": max_fee_sats,
            "timeout_seconds": 60,
        });

        let response = self
            .client
            .post(&url)
            .header("Grpc-Metadata-macaroon", &self.macaroon)
            .json(&body)
            .send()
            .await
            .map_err(LndError::from)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(LndError::Api { status, body }.into());
        }

        // /v2/router/send returns newline-delimited JSON (streaming).
        // Each line is a JSON object, possibly wrapped in {"result": ...}.
        // We scan from the last line backward to find the final SUCCEEDED update.
        let text = response.text().await.map_err(LndError::from)?;
        let lines: Vec<&str> = text
            .trim()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .collect();

        if lines.is_empty() {
            return Err(
                LndError::Payment("empty response from /v2/router/send".to_string()).into(),
            );
        }

        // Find the SUCCEEDED payment from the stream (scan backward)
        let mut succeeded: Option<PaymentUpdate> = None;
        let mut last_update: Option<PaymentUpdate> = None;

        for line in lines.iter().rev() {
            let parsed: SendPaymentResponse = serde_json::from_str(line).map_err(|e| {
                LndError::Deserialize(format!("failed to parse payment response: {e}"))
            })?;

            // Use the result wrapper if present, otherwise use flat fields
            let update = if let Some(result) = parsed.result {
                result
            } else {
                parsed.flat
            };

            if last_update.is_none() {
                last_update = Some(PaymentUpdate {
                    status: update.status.clone(),
                    payment_preimage: update.payment_preimage.clone(),
                    payment_preimage_camel: update.payment_preimage_camel.clone(),
                    payment_hash: update.payment_hash.clone(),
                    payment_hash_camel: update.payment_hash_camel.clone(),
                    value_sat: update.value_sat.clone(),
                    value_sat_camel: update.value_sat_camel.clone(),
                    value_msat: update.value_msat.clone(),
                    value_msat_camel: update.value_msat_camel.clone(),
                    fee_sat: update.fee_sat.clone(),
                    fee_sat_camel: update.fee_sat_camel.clone(),
                    fee_msat: update.fee_msat.clone(),
                    fee_msat_camel: update.fee_msat_camel.clone(),
                    failure_reason: update.failure_reason.clone(),
                });
            }

            if update.is_succeeded() {
                succeeded = Some(update);
                break;
            }
        }

        let Some(payment) = succeeded else {
            let reason = last_update
                .and_then(|u| {
                    if u.is_failed() {
                        u.failure_reason.or(u.status)
                    } else {
                        u.status
                    }
                })
                .unwrap_or_else(|| "unknown".to_string());
            return Err(LndError::Payment(reason).into());
        };

        let preimage_raw = payment.preimage().ok_or_else(|| {
            LndError::Payment("payment succeeded but returned empty preimage".to_string())
        })?;
        let hash_raw = payment.hash().ok_or_else(|| {
            LndError::Payment("payment succeeded but returned empty hash".to_string())
        })?;

        let preimage = bytes_field_to_hex(preimage_raw).map_err(ClientError::from)?;
        let payment_hash = bytes_field_to_hex(hash_raw).map_err(ClientError::from)?;

        // Verify preimage matches hash
        verify_preimage(&preimage, &payment_hash).map_err(ClientError::from)?;

        Ok(PaymentResult {
            preimage,
            payment_hash,
            amount_sats: payment.amount_sats(),
            fee_sats: payment.fee_sats(),
        })
    }

    async fn get_balance(&self) -> Result<u64, ClientError> {
        let url = format!("{}/v1/balance/channels", self.url);

        let response = self
            .client
            .get(&url)
            .header("Grpc-Metadata-macaroon", &self.macaroon)
            .send()
            .await
            .map_err(LndError::from)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(LndError::Api { status, body }.into());
        }

        let data: ChannelBalanceResponse = response
            .json()
            .await
            .map_err(|e| LndError::Deserialize(format!("failed to parse balance response: {e}")))?;

        let balance = data
            .local_balance
            .and_then(|b| b.sat)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        Ok(balance)
    }

    async fn get_info(&self) -> Result<NodeInfo, ClientError> {
        let url = format!("{}/v1/getinfo", self.url);

        let response = self
            .client
            .get(&url)
            .header("Grpc-Metadata-macaroon", &self.macaroon)
            .send()
            .await
            .map_err(LndError::from)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(LndError::Api { status, body }.into());
        }

        let data: GetInfoResponse = response
            .json()
            .await
            .map_err(|e| LndError::Deserialize(format!("failed to parse getinfo response: {e}")))?;

        Ok(NodeInfo {
            pubkey: data.identity_pubkey.unwrap_or_default(),
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
    fn bytes_field_hex_passthrough() {
        let hex_str = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";
        let result = bytes_field_to_hex(hex_str).unwrap();
        assert_eq!(result, hex_str);
    }

    #[test]
    fn bytes_field_base64_decode() {
        // 32 random bytes encoded as base64
        let bytes = [0xab_u8; 32];
        let b64 = BASE64.encode(bytes);
        let result = bytes_field_to_hex(&b64).unwrap();
        assert_eq!(result, hex::encode(bytes));
    }

    #[test]
    fn bytes_field_empty_error() {
        assert!(bytes_field_to_hex("").is_err());
    }

    #[test]
    fn bytes_field_uppercase_hex() {
        let upper = "A1B2C3D4E5F6A1B2C3D4E5F6A1B2C3D4E5F6A1B2C3D4E5F6A1B2C3D4E5F6A1B2";
        let result = bytes_field_to_hex(upper).unwrap();
        assert_eq!(result, upper.to_ascii_lowercase());
    }

    #[test]
    fn verify_preimage_valid() {
        // SHA256 of all-zero preimage
        let preimage = "00".repeat(32);
        let mut hasher = Sha256::new();
        hasher.update(hex::decode(&preimage).unwrap());
        let hash = hex::encode(hasher.finalize());
        assert!(verify_preimage(&preimage, &hash).is_ok());
    }

    #[test]
    fn verify_preimage_invalid() {
        let preimage = "00".repeat(32);
        let wrong_hash = "ff".repeat(32);
        assert!(verify_preimage(&preimage, &wrong_hash).is_err());
    }

    #[test]
    fn payment_update_snake_case() {
        let json = r#"{
            "status": "SUCCEEDED",
            "payment_preimage": "YWJj",
            "payment_hash": "ZGVm",
            "value_sat": "1000",
            "fee_sat": "5"
        }"#;
        let update: PaymentUpdate = serde_json::from_str(json).unwrap();
        assert!(update.is_succeeded());
        assert_eq!(update.preimage(), Some("YWJj"));
        assert_eq!(update.hash(), Some("ZGVm"));
        assert_eq!(update.amount_sats(), 1000);
        assert_eq!(update.fee_sats(), 5);
    }

    #[test]
    fn payment_update_camel_case() {
        let json = r#"{
            "status": "SUCCEEDED",
            "paymentPreimage": "YWJj",
            "paymentHash": "ZGVm",
            "valueSat": "2000",
            "feeSat": "10"
        }"#;
        let update: PaymentUpdate = serde_json::from_str(json).unwrap();
        assert!(update.is_succeeded());
        assert_eq!(update.preimage(), Some("YWJj"));
        assert_eq!(update.hash(), Some("ZGVm"));
        assert_eq!(update.amount_sats(), 2000);
        assert_eq!(update.fee_sats(), 10);
    }

    #[test]
    fn payment_update_millisat_values() {
        let json = r#"{
            "status": "SUCCEEDED",
            "payment_preimage": "abc",
            "payment_hash": "def",
            "value_msat": "100000",
            "fee_msat": "5000"
        }"#;
        let update: PaymentUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(update.amount_sats(), 100);
        assert_eq!(update.fee_sats(), 5);
    }

    #[test]
    fn payment_update_failed() {
        let json = r#"{
            "status": "FAILED",
            "failure_reason": "FAILURE_REASON_NO_ROUTE"
        }"#;
        let update: PaymentUpdate = serde_json::from_str(json).unwrap();
        assert!(update.is_failed());
        assert!(!update.is_succeeded());
    }

    #[test]
    fn send_payment_response_wrapped() {
        let json = r#"{"result": {"status": "SUCCEEDED", "payment_preimage": "abc", "payment_hash": "def", "value_sat": "100", "fee_sat": "1"}}"#;
        let resp: SendPaymentResponse = serde_json::from_str(json).unwrap();
        let update = resp.result.unwrap();
        assert!(update.is_succeeded());
    }

    #[test]
    fn send_payment_response_flat() {
        let json = r#"{"status": "SUCCEEDED", "payment_preimage": "abc", "payment_hash": "def", "value_sat": "100", "fee_sat": "1"}"#;
        let resp: SendPaymentResponse = serde_json::from_str(json).unwrap();
        assert!(resp.flat.is_succeeded());
    }

    #[test]
    fn channel_balance_response() {
        let json = r#"{"local_balance": {"sat": "500000"}}"#;
        let resp: ChannelBalanceResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            resp.local_balance
                .unwrap()
                .sat
                .unwrap()
                .parse::<u64>()
                .unwrap(),
            500_000
        );
    }

    #[test]
    fn get_info_response() {
        let json = r#"{"identity_pubkey": "02abc", "alias": "mynode", "num_active_channels": 5}"#;
        let resp: GetInfoResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.identity_pubkey.unwrap(), "02abc");
        assert_eq!(resp.alias.unwrap(), "mynode");
        assert_eq!(resp.num_active_channels.unwrap(), 5);
    }

    #[test]
    fn lnd_rest_backend_debug() {
        let backend = LndRestBackend::new("https://localhost:8080", "deadbeef").unwrap();
        let debug = format!("{backend:?}");
        assert!(debug.contains("LndRestBackend"));
        assert!(debug.contains("localhost:8080"));
    }

    #[test]
    fn lnd_rest_backend_trims_url() {
        let backend = LndRestBackend::new("https://localhost:8080///", "deadbeef").unwrap();
        assert_eq!(backend.url, "https://localhost:8080");
    }

    #[test]
    fn ndjson_parsing_multiline() {
        // Simulate the NDJSON stream with inflight updates
        let stream = r#"{"result":{"status":"IN_FLIGHT","payment_hash":"abc123"}}
{"result":{"status":"SUCCEEDED","payment_preimage":"deadbeef","payment_hash":"abc123","value_sat":"100","fee_sat":"2"}}"#;

        let lines: Vec<&str> = stream.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 2);

        // Verify last line has SUCCEEDED
        let last: SendPaymentResponse = serde_json::from_str(lines[1]).unwrap();
        let update = last.result.unwrap();
        assert!(update.is_succeeded());
        assert_eq!(update.amount_sats(), 100);
        assert_eq!(update.fee_sats(), 2);
    }
}
