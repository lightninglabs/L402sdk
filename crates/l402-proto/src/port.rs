//! Port definitions (hexagonal architecture).
//!
//! These traits define the boundaries of the core domain.
//! External adapters implement these traits.
//!
//! Defined in `l402-proto` (rather than `l402-core`) so that adapter
//! crates can implement them without pulling in tokio or reqwest, enabling
//! WASM compilation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::ClientError;

/// Result of a successful Lightning payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentResult {
    /// Hex-encoded payment preimage (proof of payment).
    pub preimage: String,

    /// Hex-encoded payment hash.
    pub payment_hash: String,

    /// Amount paid in satoshis (excluding fees).
    pub amount_sats: u64,

    /// Routing fee paid in satoshis.
    pub fee_sats: u64,
}

/// Information about a Lightning node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Node public key (hex-encoded).
    pub pubkey: String,

    /// Node alias.
    pub alias: String,

    /// Number of active channels.
    pub num_active_channels: u32,
}

/// Lightning Network backend port.
///
/// Implementations provide the ability to pay invoices and query node state.
/// Each backend crate (l402-lnd, l402-cln, etc.) provides an implementation.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait LnBackend: Send + Sync {
    /// Pay a BOLT11 Lightning invoice.
    ///
    /// # Arguments
    ///
    /// * `bolt11` - The BOLT11 invoice string to pay
    /// * `max_fee_sats` - Maximum routing fee in satoshis
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::PaymentFailed`] if the payment cannot be completed.
    async fn pay_invoice(
        &self,
        bolt11: &str,
        max_fee_sats: u64,
    ) -> Result<PaymentResult, ClientError>;

    /// Get the current spendable balance in satoshis.
    async fn get_balance(&self) -> Result<u64, ClientError>;

    /// Get information about the connected Lightning node.
    async fn get_info(&self) -> Result<NodeInfo, ClientError>;
}

/// Token storage port.
///
/// Implementations cache L402 tokens to avoid re-paying for the same resource.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait TokenStore: Send + Sync {
    /// Store a token for a given endpoint.
    async fn put(&self, endpoint: &str, macaroon: &str, preimage: &str) -> Result<(), ClientError>;

    /// Retrieve a cached token for an endpoint, if one exists and is still valid.
    async fn get(&self, endpoint: &str) -> Result<Option<(String, String)>, ClientError>;

    /// Remove a cached token for an endpoint.
    async fn remove(&self, endpoint: &str) -> Result<(), ClientError>;

    /// Clear all cached tokens.
    async fn clear(&self) -> Result<(), ClientError>;
}
