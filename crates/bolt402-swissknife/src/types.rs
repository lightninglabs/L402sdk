//! SwissKnife REST API request and response types.
//!
//! These types map to the SwissKnife `/v1/me/*` API endpoints.
//! Only the fields needed by the [`LnBackend`](bolt402_core::LnBackend) trait
//! are included; optional fields are deserialized but not required.

use serde::{Deserialize, Serialize};

/// Request body for `POST /v1/me/payments`.
#[derive(Debug, Serialize)]
pub(crate) struct SendPaymentRequest {
    /// The BOLT11 invoice to pay.
    pub input: String,
}

/// Response from `POST /v1/me/payments`.
#[derive(Debug, Deserialize)]
pub(crate) struct PaymentResponse {
    /// Amount paid in millisatoshis.
    pub amount_msat: u64,

    /// Fee paid in millisatoshis.
    pub fee_msat: Option<u64>,

    /// Payment status.
    pub status: PaymentStatus,

    /// Lightning-specific payment details.
    pub lightning: Option<LnPaymentDetails>,

    /// Error message (populated when status is Failed).
    pub error: Option<String>,
}

/// Lightning-specific fields within a payment response.
#[derive(Debug, Deserialize)]
pub(crate) struct LnPaymentDetails {
    /// Hex-encoded payment hash.
    pub payment_hash: String,

    /// Hex-encoded payment preimage (proof of payment).
    pub payment_preimage: Option<String>,
}

/// Payment status from SwissKnife.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum PaymentStatus {
    /// Payment completed successfully.
    Settled,
    /// Payment is still in flight.
    Pending,
    /// Payment failed.
    Failed,
    /// Any other status.
    #[serde(other)]
    Unknown,
}

/// Response from `GET /v1/me/balance`.
#[derive(Debug, Deserialize)]
pub(crate) struct BalanceResponse {
    /// Amount available to spend, in millisatoshis.
    pub available_msat: i64,
}

/// Response from `GET /v1/me` (wallet info).
#[derive(Debug, Deserialize)]
pub(crate) struct WalletResponse {
    /// Wallet UUID.
    pub id: String,

    /// Wallet user-visible name.
    #[serde(default)]
    pub user_id: String,
}

/// Error response from the SwissKnife API.
#[derive(Debug, Deserialize)]
pub(crate) struct ErrorResponse {
    /// Human-readable error message.
    pub message: Option<String>,

    /// Error status string (e.g. "UNAUTHORIZED", "NOT_FOUND").
    /// Kept for completeness with the API contract; `message` is the
    /// primary field used for error reporting.
    #[allow(dead_code)]
    pub status: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_payment_response_settled() {
        let json = r#"{
            "id": "a1b2c3d4-0000-0000-0000-000000000000",
            "wallet_id": "w1w2w3w4-0000-0000-0000-000000000000",
            "amount_msat": 100000,
            "fee_msat": 1000,
            "currency": "BTC",
            "ledger": "LIGHTNING",
            "status": "SETTLED",
            "created_at": "2026-03-15T14:00:00Z",
            "lightning": {
                "payment_hash": "abcdef1234567890",
                "payment_preimage": "fedcba0987654321"
            }
        }"#;

        let resp: PaymentResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.amount_msat, 100_000);
        assert_eq!(resp.fee_msat, Some(1000));
        assert_eq!(resp.status, PaymentStatus::Settled);

        let ln = resp.lightning.unwrap();
        assert_eq!(ln.payment_hash, "abcdef1234567890");
        assert_eq!(ln.payment_preimage.unwrap(), "fedcba0987654321");
    }

    #[test]
    fn deserialize_payment_response_failed() {
        let json = r#"{
            "id": "a1b2c3d4-0000-0000-0000-000000000000",
            "wallet_id": "w1w2w3w4-0000-0000-0000-000000000000",
            "amount_msat": 0,
            "status": "FAILED",
            "currency": "BTC",
            "ledger": "LIGHTNING",
            "error": "no route found",
            "created_at": "2026-03-15T14:00:00Z"
        }"#;

        let resp: PaymentResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, PaymentStatus::Failed);
        assert_eq!(resp.error.unwrap(), "no route found");
    }

    #[test]
    fn deserialize_balance_response() {
        let json = r#"{ "received_msat": 1000000, "sent_msat": 100000, "fees_paid_msat": 500, "available_msat": 899500 }"#;
        let resp: BalanceResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.available_msat, 899_500);
    }

    #[test]
    fn deserialize_wallet_response() {
        let json = r#"{ "id": "wallet-uuid", "user_id": "auth0|user123", "currency": "BTC" }"#;
        let resp: WalletResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "wallet-uuid");
        assert_eq!(resp.user_id, "auth0|user123");
    }

    #[test]
    fn deserialize_error_response() {
        let json = r#"{ "message": "Unauthorized", "status": "UNAUTHORIZED" }"#;
        let resp: ErrorResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.message.unwrap(), "Unauthorized");
    }

    #[test]
    fn serialize_send_payment_request() {
        let req = SendPaymentRequest {
            input: "lnbc100n1test".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"input\":\"lnbc100n1test\""));
    }

    #[test]
    fn payment_status_unknown_variant() {
        let json = r#""SOME_NEW_STATUS""#;
        let status: PaymentStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, PaymentStatus::Unknown);
    }
}
