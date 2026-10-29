//! Budget tracking for L402 payments.
//!
//! Prevents runaway spending by enforcing limits at multiple granularities:
//! per-request, hourly, daily, and total budget caps.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use web_time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use l402_proto::ClientError;

/// Budget configuration with multiple limit granularities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    /// Maximum fee-inclusive amount per individual payment (satoshis).
    pub per_request_max: Option<u64>,

    /// Maximum fee-inclusive amount per hour (satoshis).
    pub hourly_max: Option<u64>,

    /// Maximum fee-inclusive amount per day (satoshis).
    pub daily_max: Option<u64>,

    /// Maximum total amount (satoshis).
    pub total_max: Option<u64>,

    /// Additional domain limits; these never relax the global budget.
    #[serde(default)]
    pub domain_budgets: HashMap<String, Budget>,
}

impl Budget {
    /// Create an unlimited budget (no restrictions).
    pub fn unlimited() -> Self {
        Self {
            per_request_max: None,
            hourly_max: None,
            daily_max: None,
            total_max: None,
            domain_budgets: HashMap::new(),
        }
    }

    /// Check if a payment of `amount` satoshis is allowed under this budget.
    ///
    /// Returns `Ok(())` if allowed, or [`ClientError::BudgetExceeded`] if any limit is violated.
    pub fn check(&self, amount: u64) -> Result<(), ClientError> {
        if let Some(max) = self.per_request_max
            && amount > max
        {
            return Err(ClientError::BudgetExceeded {
                reason: format!("payment of {amount} sats exceeds per-request limit of {max} sats"),
            });
        }
        Ok(())
    }
}

/// Budget tracker that enforces spending limits over time.
#[derive(Debug, Clone)]
pub struct BudgetTracker {
    budget: Budget,
    state: Arc<RwLock<BudgetState>>,
}

#[derive(Debug, Default)]
struct Ledger {
    total: u64,
    hourly: HashMap<u64, u64>,
    daily: HashMap<u64, u64>,
}

impl Ledger {
    fn allowance(
        &self,
        budget: &Budget,
        amount: u64,
        pending: u64,
        now: u64,
    ) -> Result<u64, ClientError> {
        let mut allowance = u64::MAX;
        for (spent, limit) in [
            (0, budget.per_request_max),
            (self.total.saturating_add(pending), budget.total_max),
            (
                self.hourly
                    .get(&(now / 3600))
                    .copied()
                    .unwrap_or(0)
                    .saturating_add(pending),
                budget.hourly_max,
            ),
            (
                self.daily
                    .get(&(now / 86400))
                    .copied()
                    .unwrap_or(0)
                    .saturating_add(pending),
                budget.daily_max,
            ),
            // Always prevent accounting overflow, even with an unlimited budget.
            (self.total.saturating_add(pending), Some(u64::MAX)),
        ] {
            if let Some(limit) = limit {
                let remaining = limit
                    .checked_sub(spent)
                    .and_then(|v| v.checked_sub(amount))
                    .ok_or_else(|| ClientError::BudgetExceeded {
                        reason: "invoice and reserved spending exceed budget".to_string(),
                    })?;
                allowance = allowance.min(remaining);
            }
        }
        Ok(allowance)
    }

    fn record(&mut self, amount: u64, now: u64) {
        self.total = self.total.saturating_add(amount);
        let hourly = self.hourly.entry(now / 3600).or_default();
        *hourly = hourly.saturating_add(amount);
        let daily = self.daily.entry(now / 86400).or_default();
        *daily = daily.saturating_add(amount);
        self.hourly
            .retain(|&k, _| k >= (now / 3600).saturating_sub(48));
        self.daily
            .retain(|&k, _| k >= (now / 86400).saturating_sub(2));
    }
}

#[derive(Debug, Default)]
struct BudgetState {
    global: Ledger,
    domains: HashMap<String, Ledger>,
    pending: HashMap<u64, (u64, Option<String>)>,
    next_id: u64,
    frozen: bool,
}

/// A reservation stays charged if its payment future is dropped or the outcome is unknown.
#[derive(Debug)]
pub(crate) struct Reservation {
    id: u64,
    pub(crate) max_fee_sats: u64,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl BudgetTracker {
    /// Create a new budget tracker with the given budget configuration.
    pub fn new(budget: Budget) -> Self {
        Self {
            budget,
            state: Arc::new(RwLock::new(BudgetState::default())),
        }
    }

    pub(crate) fn reserve(
        &self,
        amount: u64,
        max_fee: u64,
        domain: Option<&str>,
    ) -> Result<Reservation, ClientError> {
        self.reserve_at(amount, max_fee, domain, now_secs())
    }

    fn reserve_at(
        &self,
        amount: u64,
        max_fee: u64,
        domain: Option<&str>,
        now: u64,
    ) -> Result<Reservation, ClientError> {
        let mut state = self.state.write().expect("RwLock poisoned");
        if state.frozen {
            return Err(ClientError::BudgetExceeded {
                reason: "backend exceeded its reservation; further payments are disabled"
                    .to_string(),
            });
        }
        let pending = state.pending.values().map(|(amount, _)| amount).sum();
        let mut fee = max_fee.min(state.global.allowance(&self.budget, amount, pending, now)?);
        if let Some(domain) = domain
            && let Some(budget) = self.budget.domain_budgets.get(domain)
        {
            let pending = state
                .pending
                .values()
                .filter(|(_, d)| d.as_deref() == Some(domain))
                .map(|(amount, _)| amount)
                .sum();
            let empty = Ledger::default();
            let ledger = state.domains.get(domain).unwrap_or(&empty);
            fee = fee.min(ledger.allowance(budget, amount, pending, now)?);
        }
        let id = state.next_id;
        state.next_id = id
            .checked_add(1)
            .ok_or_else(|| ClientError::BudgetExceeded {
                reason: "reservation IDs exhausted".to_string(),
            })?;
        // allowance includes the u64 ceiling, so this sum cannot overflow.
        state
            .pending
            .insert(id, (amount + fee, domain.map(str::to_string)));
        Ok(Reservation {
            id,
            max_fee_sats: fee,
        })
    }

    #[allow(clippy::needless_pass_by_value)] // Consume the token to prevent double release.
    pub(crate) fn release(&self, reservation: Reservation) {
        self.state
            .write()
            .expect("RwLock poisoned")
            .pending
            .remove(&reservation.id);
    }

    pub(crate) fn settle(&self, reservation: Reservation, amount: u64) -> Result<(), ClientError> {
        self.settle_at(reservation, amount, now_secs(), false)
    }

    pub(crate) fn settle_payment(
        &self,
        reservation: Reservation,
        amount: u64,
        violation: bool,
    ) -> Result<(), ClientError> {
        self.settle_at(reservation, amount, now_secs(), violation)
    }

    #[allow(clippy::needless_pass_by_value)] // Consume the token to prevent double settlement.
    fn settle_at(
        &self,
        reservation: Reservation,
        amount: u64,
        now: u64,
        violation: bool,
    ) -> Result<(), ClientError> {
        let mut state = self.state.write().expect("RwLock poisoned");
        let (reserved, domain) = state
            .pending
            .remove(&reservation.id)
            .expect("live reservation");
        // Record even a backend violation: paid money must never disappear.
        state.global.record(amount, now);
        if let Some(domain) = domain {
            state.domains.entry(domain).or_default().record(amount, now);
        }
        if violation || amount > reserved {
            state.frozen = true;
            return Err(ClientError::BudgetExceeded {
                reason: "settled payment exceeded its reservation".to_string(),
            });
        }
        Ok(())
    }

    /// Atomically check and record a final, fee-inclusive amount.
    ///
    /// Domain limits apply in addition to global limits, with separate domain counters.
    #[allow(clippy::unused_async, clippy::unused_async_trait_impl)]
    pub async fn check_and_record(
        &self,
        amount: u64,
        domain: Option<&str>,
    ) -> Result<(), ClientError> {
        let reservation = self.reserve(amount, 0, domain)?;
        self.settle(reservation, amount)
    }

    /// Get settled spending plus outstanding reservations, including routing fees.
    /// Unknown or cancelled payments keep their reservation until this tracker is dropped.
    #[allow(clippy::unused_async, clippy::unused_async_trait_impl)]
    pub async fn total_spent(&self) -> u64 {
        let state = self.state.read().expect("RwLock poisoned");
        state
            .pending
            .values()
            .fold(state.global.total, |total, (amount, _)| {
                total.saturating_add(*amount)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_request_limit() {
        let budget = Budget {
            per_request_max: Some(100),
            hourly_max: None,
            daily_max: None,
            total_max: None,
            domain_budgets: HashMap::new(),
        };

        assert!(budget.check(50).is_ok());
        assert!(budget.check(100).is_ok());
        assert!(budget.check(101).is_err());
    }

    #[tokio::test]
    async fn hourly_limit() {
        let budget = Budget {
            per_request_max: None,
            hourly_max: Some(1000),
            daily_max: None,
            total_max: None,
            domain_budgets: HashMap::new(),
        };

        let tracker = BudgetTracker::new(budget);

        assert!(tracker.check_and_record(500, None).await.is_ok());
        assert!(tracker.check_and_record(499, None).await.is_ok());
        assert!(tracker.check_and_record(2, None).await.is_err());
    }

    #[tokio::test]
    async fn accounting_overflow_fails_without_mutation() {
        for total_max in [None, Some(u64::MAX)] {
            let mut budget = Budget::unlimited();
            budget.total_max = total_max;
            let tracker = BudgetTracker::new(budget);
            tracker.check_and_record(u64::MAX, None).await.unwrap();
            assert!(tracker.check_and_record(1, None).await.is_err());
            assert_eq!(tracker.total_spent().await, u64::MAX);
            assert!(
                tracker
                    .state
                    .read()
                    .unwrap()
                    .global
                    .hourly
                    .values()
                    .all(|v| *v == u64::MAX)
            );
        }
    }

    #[tokio::test]
    async fn total_limit() {
        let budget = Budget {
            per_request_max: None,
            hourly_max: None,
            daily_max: None,
            total_max: Some(5000),
            domain_budgets: HashMap::new(),
        };

        let tracker = BudgetTracker::new(budget);

        for _ in 0..5 {
            tracker.check_and_record(1000, None).await.unwrap();
        }

        assert!(tracker.check_and_record(1, None).await.is_err());
        assert_eq!(tracker.total_spent().await, 5000);
    }
    #[tokio::test]
    async fn pending_reservations_survive_hour_and_day_rollover() {
        let tracker = BudgetTracker::new(Budget {
            hourly_max: Some(150),
            daily_max: Some(150),
            ..Budget::unlimited()
        });
        let reservation = tracker.reserve_at(100, 10, None, 86_399).unwrap();
        assert!(tracker.reserve_at(50, 10, None, 86_400).is_err());
        tracker.settle_at(reservation, 103, 86_400, false).unwrap();
        assert!(tracker.reserve_at(48, 0, None, 86_401).is_err());
        let next = tracker.reserve_at(47, 10, None, 86_401).unwrap();
        assert_eq!(next.max_fee_sats, 0);
        tracker.release(next);
        assert_eq!(tracker.total_spent().await, 103);
    }

    #[tokio::test]
    async fn domain_limits_are_independent_and_cannot_relax_global_limits() {
        let mut budget = Budget {
            total_max: Some(250),
            ..Budget::unlimited()
        };
        budget.domain_budgets.insert(
            "a".into(),
            Budget {
                total_max: Some(100),
                ..Budget::unlimited()
            },
        );
        budget
            .domain_budgets
            .insert("b".into(), Budget::unlimited());
        let tracker = BudgetTracker::new(budget);
        tracker.check_and_record(100, Some("a")).await.unwrap();
        assert!(tracker.check_and_record(1, Some("a")).await.is_err());
        tracker.check_and_record(150, Some("b")).await.unwrap();
        assert!(tracker.check_and_record(1, Some("b")).await.is_err());
        assert_eq!(tracker.total_spent().await, 250);
    }

    #[tokio::test]
    async fn arithmetic_boundaries_cannot_wrap_fee_reservations() {
        let tracker = BudgetTracker::new(Budget::unlimited());
        let reservation = tracker.reserve(u64::MAX - 2, u64::MAX, None).unwrap();
        assert_eq!(reservation.max_fee_sats, 2);
        assert_eq!(tracker.total_spent().await, u64::MAX);
        assert!(tracker.reserve(1, 0, None).is_err());
        tracker.release(reservation);
        assert_eq!(tracker.total_spent().await, 0);
    }

    #[test]
    fn concurrent_reservations_share_one_atomic_limit() {
        let tracker = BudgetTracker::new(Budget {
            total_max: Some(100),
            ..Budget::unlimited()
        });
        let handles: Vec<_> = (0..20)
            .map(|_| {
                let tracker = tracker.clone();
                std::thread::spawn(move || tracker.reserve(10, 0, None).is_ok())
            })
            .collect();
        assert_eq!(
            handles
                .into_iter()
                .map(|h| h.join().unwrap())
                .filter(|ok| *ok)
                .count(),
            10
        );
    }
}
