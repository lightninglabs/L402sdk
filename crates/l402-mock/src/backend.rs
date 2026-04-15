//! Mock Lightning backend that "pays" by looking up preimages.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use l402_proto::ClientError;
use l402_proto::port::{LnBackend, NodeInfo, PaymentResult};
use tokio::sync::RwLock;

use crate::challenge::PendingChallenge;

/// A mock Lightning backend for testing.
///
/// Instead of making real Lightning payments, it looks up the preimage
/// from the mock server's challenge registry. This simulates a successful
/// payment without any real Lightning infrastructure.
#[derive(Debug, Clone)]
pub struct MockLnBackend {
    challenges: Arc<RwLock<HashMap<String, PendingChallenge>>>,
    balance: Arc<RwLock<u64>>,
}

impl MockLnBackend {
    /// Create a new mock backend connected to a challenge registry.
    pub fn new(challenges: Arc<RwLock<HashMap<String, PendingChallenge>>>) -> Self {
        Self {
            challenges,
            balance: Arc::new(RwLock::new(1_000_000)), // 1M sats default
        }
    }

    /// Set the simulated balance.
    pub async fn set_balance(&self, sats: u64) {
        *self.balance.write().await = sats;
    }
}

#[async_trait]
impl LnBackend for MockLnBackend {
    async fn pay_invoice(
        &self,
        bolt11: &str,
        _max_fee_sats: u64,
    ) -> Result<PaymentResult, ClientError> {
        let challenges = self.challenges.read().await;

        // Find the challenge matching this invoice
        let challenge = challenges
            .get(bolt11)
            .ok_or_else(|| ClientError::PaymentFailed {
                reason: format!("unknown invoice: {bolt11}"),
            })?;

        // Deduct from balance
        let mut balance = self.balance.write().await;
        if *balance < challenge.amount_sats {
            return Err(ClientError::PaymentFailed {
                reason: format!(
                    "insufficient balance: have {} sats, need {}",
                    *balance, challenge.amount_sats
                ),
            });
        }
        *balance -= challenge.amount_sats;

        Ok(PaymentResult {
            preimage: challenge.preimage.clone(),
            payment_hash: challenge.payment_hash.clone(),
            amount_sats: challenge.amount_sats,
            fee_sats: 0,
        })
    }

    async fn get_balance(&self) -> Result<u64, ClientError> {
        Ok(*self.balance.read().await)
    }

    async fn get_info(&self) -> Result<NodeInfo, ClientError> {
        Ok(NodeInfo {
            pubkey: "mock_pubkey_000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            alias: "l402-mock".to_string(),
            num_active_channels: 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pay_known_invoice() {
        let challenges = Arc::new(RwLock::new(HashMap::new()));
        let challenge = PendingChallenge::generate(100);
        challenges
            .write()
            .await
            .insert(challenge.invoice.clone(), challenge.clone());

        let backend = MockLnBackend::new(challenges);
        let result = backend.pay_invoice(&challenge.invoice, 10).await.unwrap();

        assert_eq!(result.preimage, challenge.preimage);
        assert_eq!(result.amount_sats, 100);
        assert_eq!(result.fee_sats, 0);
    }

    #[tokio::test]
    async fn reject_unknown_invoice() {
        let challenges = Arc::new(RwLock::new(HashMap::new()));
        let backend = MockLnBackend::new(challenges);

        let result = backend.pay_invoice("lnbc999n1unknown", 10).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn insufficient_balance() {
        let challenges = Arc::new(RwLock::new(HashMap::new()));
        let challenge = PendingChallenge::generate(100);
        challenges
            .write()
            .await
            .insert(challenge.invoice.clone(), challenge.clone());

        let backend = MockLnBackend::new(challenges);
        backend.set_balance(50).await; // Only 50 sats

        let result = backend.pay_invoice(&challenge.invoice, 10).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn balance_decreases_after_payment() {
        let challenges = Arc::new(RwLock::new(HashMap::new()));
        let challenge = PendingChallenge::generate(100);
        challenges
            .write()
            .await
            .insert(challenge.invoice.clone(), challenge.clone());

        let backend = MockLnBackend::new(challenges);
        assert_eq!(backend.get_balance().await.unwrap(), 1_000_000);

        backend.pay_invoice(&challenge.invoice, 10).await.unwrap();
        assert_eq!(backend.get_balance().await.unwrap(), 999_900);
    }
}
