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
    /// Maximum amount per individual payment (satoshis).
    pub per_request_max: Option<u64>,

    /// Maximum amount per hour (satoshis).
    pub hourly_max: Option<u64>,

    /// Maximum amount per day (satoshis).
    pub daily_max: Option<u64>,

    /// Maximum total amount (satoshis).
    pub total_max: Option<u64>,

    /// Domain-specific budget overrides.
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

#[derive(Debug)]
struct BudgetState {
    total: u64,
    hourly: HashMap<u64, u64>, // hour_timestamp -> amount
    daily: HashMap<u64, u64>,  // day_timestamp -> amount
}

impl BudgetTracker {
    /// Create a new budget tracker with the given budget configuration.
    pub fn new(budget: Budget) -> Self {
        Self {
            budget,
            state: Arc::new(RwLock::new(BudgetState {
                total: 0,
                hourly: HashMap::new(),
                daily: HashMap::new(),
            })),
        }
    }

    /// Check if a payment is allowed and record it if successful.
    #[allow(clippy::unused_async, clippy::unused_async_trait_impl)] // Kept async for API consistency; callers .await this
    pub async fn check_and_record(
        &self,
        amount: u64,
        domain: Option<&str>,
    ) -> Result<(), ClientError> {
        // Apply domain-specific budget if available
        let effective_budget = if let Some(domain) = domain {
            if let Some(domain_budget) = self.budget.domain_budgets.get(domain) {
                domain_budget.clone()
            } else {
                self.budget.clone()
            }
        } else {
            self.budget.clone()
        };

        effective_budget.check(amount)?;

        let mut state = self.state.write().expect("RwLock poisoned");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_secs();
        let current_hour = now / 3600;
        let current_day = now / 86400;

        // Compute all counters first: neither overflow nor a failed limit check
        // may partially mutate the reservation.
        let checked_total = |spent: u64| {
            spent
                .checked_add(amount)
                .ok_or_else(|| ClientError::BudgetExceeded {
                    reason: "payment would overflow budget accounting".to_string(),
                })
        };
        let hourly = checked_total(state.hourly.get(&current_hour).copied().unwrap_or(0))?;
        let daily = checked_total(state.daily.get(&current_day).copied().unwrap_or(0))?;
        let total = checked_total(state.total)?;
        for (name, next, limit) in [
            ("hourly", hourly, effective_budget.hourly_max),
            ("daily", daily, effective_budget.daily_max),
            ("total", total, effective_budget.total_max),
        ] {
            if let Some(limit) = limit
                && next > limit
            {
                return Err(ClientError::BudgetExceeded {
                    reason: format!("payment would exceed {name} limit ({next} > {limit})"),
                });
            }
        }
        state.total = total;
        state.hourly.insert(current_hour, hourly);
        state.daily.insert(current_day, daily);

        // Clean up old entries (older than 48 hours)
        let cutoff_hour = current_hour.saturating_sub(48);
        state.hourly.retain(|&k, _| k >= cutoff_hour);
        let cutoff_day = current_day.saturating_sub(2);
        state.daily.retain(|&k, _| k >= cutoff_day);

        Ok(())
    }

    /// Get the total amount spent so far.
    #[allow(clippy::unused_async, clippy::unused_async_trait_impl)] // Kept async for API consistency; callers .await this
    pub async fn total_spent(&self) -> u64 {
        self.state.read().expect("RwLock poisoned").total
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
}
