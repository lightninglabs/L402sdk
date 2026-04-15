//! SwissKnife REST API request and response types.
//!
//! These types map to the SwissKnife `/v1/me/*` API endpoints.
//! Only the fields needed by the [`LnBackend`](l402_proto::port::LnBackend) trait
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

    /// Hex-encoded payment hash.
    pub payment_hash: Option<String>,

    /// Hex-encoded payment preimage (proof of payment).
    pub payment_preimage: Option<String>,

    /// Error message (populated when status is Failed).
    pub error: Option<String>,
}

/// Payment status from SwissKnife.
#[derive(Debug, Deserialize, PartialEq, Eq)]
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
    /// Human-readable error reason.
    pub reason: Option<String>,

    /// Error status string (e.g. "401 Unauthorized").
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
            "fee_msat": 0,
            "currency": "Regtest",
            "ledger": "Lightning",
            "status": "Settled",
            "created_at": "2026-03-15T14:00:00Z",
            "payment_hash": "abcdef1234567890",
            "payment_preimage": "fedcba0987654321"
        }"#;

        let resp: PaymentResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.amount_msat, 100_000);
        assert_eq!(resp.fee_msat, Some(0));
        assert_eq!(resp.status, PaymentStatus::Settled);
        assert_eq!(resp.payment_hash.unwrap(), "abcdef1234567890");
        assert_eq!(resp.payment_preimage.unwrap(), "fedcba0987654321");
    }

    #[test]
    fn deserialize_payment_response_failed() {
        let json = r#"{
            "id": "a1b2c3d4-0000-0000-0000-000000000000",
            "wallet_id": "w1w2w3w4-0000-0000-0000-000000000000",
            "amount_msat": 0,
            "status": "Failed",
            "currency": "Regtest",
            "ledger": "Lightning",
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
        let json = r#"{ "reason": "Unauthorized", "status": "401 Unauthorized" }"#;
        let resp: ErrorResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.reason.unwrap(), "Unauthorized");
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
        let json = r#""SomeNewStatus""#;
        let status: PaymentStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, PaymentStatus::Unknown);
    }
}
