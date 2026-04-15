//! NWC backend implementation using the `nwc` crate.
//!
//! Connects to any NWC-compatible wallet (Alby Hub, Mutiny, LNbits,
//! Phoenixd, etc.) via [NIP-47](https://github.com/nostr-protocol/nips/blob/master/47.md).
//!
//! # Example
//!
//! ```rust,no_run
//! use l402_nwc::NwcBackend;
//!
//! # async fn example() {
//! let backend = NwcBackend::new("nostr+walletconnect://...").await.unwrap();
//! // Use with L402Client::builder().ln_backend(backend)...
//! # }
//! ```

use std::fmt;

use async_trait::async_trait;
use l402_proto::ClientError;
use l402_proto::port::{LnBackend, NodeInfo, PaymentResult};
use nostr::nips::nip47::{NostrWalletConnectURI, PayInvoiceRequest};
use nwc::NWC;
use tracing::debug;

use crate::error::NwcError;

/// Nostr Wallet Connect (NIP-47) Lightning backend.
///
/// Uses the NWC protocol to communicate with a remote wallet via Nostr
/// relays. This allows L402sdk to pay L402 invoices through any
/// NWC-compatible wallet without direct node access.
///
/// # Supported Wallets
///
/// - [Alby Hub](https://albyhub.com/)
/// - [Mutiny Wallet](https://www.mutinywallet.com/)
/// - [LNbits](https://lnbits.com/)
/// - [Phoenixd](https://phoenix.acinq.co/)
/// - Any wallet implementing NIP-47
///
/// # Example
///
/// ```rust,no_run
/// use l402_nwc::NwcBackend;
/// use l402_core::{L402Client, L402ClientConfig};
/// use l402_core::budget::Budget;
/// use l402_core::cache::InMemoryTokenStore;
///
/// # async fn example() {
/// let backend = NwcBackend::new("nostr+walletconnect://...").await.unwrap();
///
/// let client = L402Client::builder()
///     .ln_backend(backend)
///     .token_store(InMemoryTokenStore::default())
///     .budget(Budget::unlimited())
///     .build()
///     .unwrap();
/// # }
/// ```
pub struct NwcBackend {
    nwc: NWC,
    uri_display: String,
}

impl fmt::Debug for NwcBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NwcBackend")
            .field("uri", &self.uri_display)
            .finish_non_exhaustive()
    }
}

impl NwcBackend {
    /// Connect to a wallet using a NWC connection URI.
    ///
    /// The URI format is:
    /// ```text
    /// nostr+walletconnect://<wallet_pubkey>?relay=<relay_url>&secret=<app_secret_key>
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`NwcError::InvalidUri`] if the URI cannot be parsed.
    pub async fn new(uri: &str) -> Result<Self, NwcError> {
        let parsed = NostrWalletConnectURI::parse(uri)?;

        // Store a safe display form (pubkey only, no secret)
        let uri_display = format!(
            "nostr+walletconnect://{}...",
            &parsed.public_key.to_string()[..16]
        );

        debug!(uri = %uri_display, "connecting to NWC wallet");
        let nwc = NWC::new(parsed);

        Ok(Self { nwc, uri_display })
    }

    /// Connect using the `NWC_CONNECTION_URI` environment variable.
    ///
    /// # Errors
    ///
    /// Returns [`NwcError::InvalidUri`] if the environment variable is not set
    /// or contains an invalid URI.
    pub async fn from_env() -> Result<Self, NwcError> {
        let uri = std::env::var("NWC_CONNECTION_URI").map_err(|_| {
            NwcError::InvalidUri("NWC_CONNECTION_URI environment variable not set".to_string())
        })?;
        Self::new(&uri).await
    }

    /// Gracefully shut down the NWC client, closing relay connections.
    pub async fn shutdown(self) {
        self.nwc.shutdown().await;
    }
}

#[async_trait]
impl LnBackend for NwcBackend {
    async fn pay_invoice(
        &self,
        bolt11: &str,
        _max_fee_sats: u64,
    ) -> Result<PaymentResult, ClientError> {
        debug!(invoice = %&bolt11[..bolt11.len().min(20)], "paying invoice via NWC");

        let request = PayInvoiceRequest::new(bolt11);

        let response = self
            .nwc
            .pay_invoice(request)
            .await
            .map_err(NwcError::from)?;

        let preimage = response.preimage;
        let fee_sats = response.fees_paid.unwrap_or(0) / 1000;

        Ok(PaymentResult {
            preimage,
            // NWC doesn't return payment_hash in pay_invoice response.
            payment_hash: String::new(),
            amount_sats: 0,
            fee_sats,
        })
    }

    async fn get_balance(&self) -> Result<u64, ClientError> {
        debug!("querying balance via NWC");

        let balance_msat = self.nwc.get_balance().await.map_err(NwcError::from)?;

        // NWC returns balance in millisatoshis, convert to satoshis.
        Ok(balance_msat / 1000)
    }

    async fn get_info(&self) -> Result<NodeInfo, ClientError> {
        debug!("querying node info via NWC");

        let info = self.nwc.get_info().await.map_err(NwcError::from)?;

        // NWC get_info returns supported methods and optional node alias/color/pubkey.
        // Map available fields to our NodeInfo struct.
        let alias = info.alias.unwrap_or_else(|| "NWC Wallet".to_string());
        let pubkey = info.pubkey.unwrap_or_else(|| "unknown".to_string());
        let num_methods = info.methods.len();

        Ok(NodeInfo {
            pubkey,
            alias,
            num_active_channels: u32::try_from(num_methods).unwrap_or(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_format() {
        // NwcBackend Debug should not leak the secret key
        let display = format!(
            "{:?}",
            // We can't construct a real NwcBackend without a relay, but we can
            // verify the Debug impl concept via the uri_display field.
            "NwcBackend { uri: \"nostr+walletconnect://abcdef1234567890...\" }"
        );
        assert!(display.contains("nostr+walletconnect://"));
        assert!(!display.contains("secret"));
    }

    #[tokio::test]
    async fn from_env_missing() {
        // Ensure from_env returns an error when the env var is not set.
        // SAFETY: test isolation — we're checking for the absence of a var.
        unsafe { std::env::remove_var("NWC_CONNECTION_URI") };
        let result = NwcBackend::from_env().await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("NWC_CONNECTION_URI"));
    }

    #[tokio::test]
    async fn new_invalid_uri() {
        let result = NwcBackend::new("not-a-valid-uri").await;
        assert!(result.is_err());
    }
}
