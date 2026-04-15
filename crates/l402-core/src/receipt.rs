//! Payment receipts for audit and cost analysis.

use serde::{Deserialize, Serialize};
use web_time::{SystemTime, UNIX_EPOCH};

/// A structured receipt for an L402 payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    /// Unix timestamp (seconds) of the payment.
    pub timestamp: u64,

    /// The endpoint that was accessed.
    pub endpoint: String,

    /// Amount paid in satoshis (excluding routing fees).
    pub amount_sats: u64,

    /// Routing fee paid in satoshis.
    pub fee_sats: u64,

    /// Hex-encoded payment hash.
    pub payment_hash: String,

    /// Hex-encoded preimage (proof of payment).
    pub preimage: String,

    /// HTTP response status code after presenting the L402 token.
    pub response_status: u16,

    /// Total latency from initial request to final response (milliseconds).
    pub latency_ms: u64,
}

impl Receipt {
    /// Create a new receipt with the current timestamp.
    pub fn new(
        endpoint: String,
        amount_sats: u64,
        fee_sats: u64,
        payment_hash: String,
        preimage: String,
        response_status: u16,
        latency_ms: u64,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_secs();

        Self {
            timestamp,
            endpoint,
            amount_sats,
            fee_sats,
            payment_hash,
            preimage,
            response_status,
            latency_ms,
        }
    }

    /// Get the total cost (amount + routing fee) in satoshis.
    pub fn total_cost_sats(&self) -> u64 {
        self.amount_sats + self.fee_sats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_cost() {
        let receipt = Receipt::new(
            "https://api.example.com".to_string(),
            100,
            5,
            "abc123".to_string(),
            "def456".to_string(),
            200,
            450,
        );

        assert_eq!(receipt.total_cost_sats(), 105);
        assert!(receipt.timestamp > 0);
    }
}
