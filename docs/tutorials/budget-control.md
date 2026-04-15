# Budget Control for Autonomous Agents

When AI agents spend real Bitcoin, you need guardrails. L402sdk's budget system is built into the protocol flow, ensuring limits are checked before any payment is attempted.

## Why Budgets Matter

An AI agent with access to a Lightning node can pay any L402-gated API it encounters. Without limits:
- A malicious API could charge excessive amounts
- A loop could drain your node
- A single misbehaving agent could burn through your entire balance

The `BudgetTracker` prevents all of this.

## Budget Configuration

The `Budget` struct supports four types of limits:

```rust
use l402_core::budget::Budget;

let budget = Budget {
    per_request_max: Some(1_000),     // Max 1,000 sats per payment
    hourly_max: Some(10_000),         // Max 10,000 sats per hour
    daily_max: Some(100_000),         // Max 100,000 sats per day
    total_max: Some(1_000_000),       // Max 1,000,000 sats lifetime
    domain_budgets: Default::default(),
};
```

All limits are optional. Omitted limits are unchecked. `Budget::unlimited()` creates a budget with no restrictions.

### Limit Precedence

All limits are checked independently. If any limit would be exceeded, the payment is rejected with `ClientError::BudgetExceeded`.

Example: with `per_request_max: 500` and `hourly_max: 2000`, you could make four 500-sat payments per hour, but a single 501-sat payment would be rejected.

## Domain-Specific Budgets

You can set different limits for different API providers:

```rust
use std::collections::HashMap;

let mut domain_budgets = HashMap::new();

// Generous budget for a trusted internal API
domain_budgets.insert(
    "api.mycompany.com".to_string(),
    Budget {
        per_request_max: Some(10_000),
        daily_max: Some(500_000),
        ..Budget::unlimited()
    },
);

// Tight budget for an untrusted third-party API
domain_budgets.insert(
    "sketchy-api.example.com".to_string(),
    Budget {
        per_request_max: Some(100),
        daily_max: Some(1_000),
        ..Budget::unlimited()
    },
);

let budget = Budget {
    per_request_max: Some(1_000),    // Default for unknown domains
    daily_max: Some(50_000),
    total_max: Some(1_000_000),
    hourly_max: None,
    domain_budgets,
};
```

When a request is made, the budget tracker checks for a domain-specific budget first. If none exists, it falls back to the default limits.

## Using Budgets with L402Client

```rust
use l402_core::L402Client;
use l402_core::budget::Budget;
use l402_core::cache::InMemoryTokenStore;

let client = L402Client::builder()
    .ln_backend(my_backend)
    .token_store(InMemoryTokenStore::default())
    .budget(Budget {
        per_request_max: Some(500),
        daily_max: Some(50_000),
        ..Budget::unlimited()
    })
    .build()
    .unwrap();

// This will fail if the invoice amount exceeds 500 sats
match client.get("https://expensive-api.com/data").await {
    Ok(response) => println!("Got data: {}", response.status()),
    Err(l402_proto::ClientError::BudgetExceeded { reason }) => {
        println!("Payment blocked: {reason}");
    }
    Err(e) => println!("Other error: {e}"),
}
```

## Using Budgets with Vercel AI SDK

```typescript
import { createL402Tools } from 'l402-ai-sdk';
import init, { WasmBudgetConfig, WasmL402Client } from 'l402-wasm';

await init();

const client = WasmL402Client.withLndRest(
  'https://localhost:8080',
  process.env.LND_MACAROON!,
  new WasmBudgetConfig(1_000, 10_000, 100_000, 1_000_000),
  100,
);

const tools = createL402Tools({ client });
```

The AI agent will receive a clear error message if it tries to exceed the budget, and can report this to the user.

## Receipt-Based Cost Analysis

Every successful payment generates a `Receipt`. Use receipts to understand spending patterns:

```rust
let receipts = client.receipts().await;

// Total spent
let total: u64 = receipts.iter().map(|r| r.total_cost_sats()).sum();
println!("Total spent: {total} sats");

// Most expensive endpoint
if let Some(max) = receipts.iter().max_by_key(|r| r.total_cost_sats()) {
    println!("Most expensive: {} ({} sats)", max.endpoint, max.total_cost_sats());
}

// Average latency
let avg_latency = receipts.iter().map(|r| r.latency_ms).sum::<u64>()
    / receipts.len().max(1) as u64;
println!("Average latency: {avg_latency}ms");

// Spend by domain
use std::collections::HashMap;
let mut by_domain: HashMap<&str, u64> = HashMap::new();
for r in &receipts {
    if let Ok(url) = reqwest::Url::parse(&r.endpoint) {
        if let Some(host) = url.host_str() {
            *by_domain.entry(host).or_default() += r.total_cost_sats();
        }
    }
}
for (domain, spent) in &by_domain {
    println!("  {domain}: {spent} sats");
}
```

## Production Recommendations

**For development and testing:**
```rust
Budget::unlimited()
```

**For a supervised agent (human reviews outputs):**
```rust
Budget {
    per_request_max: Some(5_000),     // Reasonable per-request cap
    daily_max: Some(100_000),          // ~$50 at current prices
    ..Budget::unlimited()
}
```

**For a fully autonomous agent:**
```rust
Budget {
    per_request_max: Some(1_000),      // Tight per-request cap
    hourly_max: Some(5_000),           // Rate-limit spending
    daily_max: Some(25_000),           // Hard daily cap
    total_max: Some(500_000),          // Lifetime cap, require reset
    domain_budgets: trusted_domains,   // Higher limits for known APIs
}
```

**Key principle:** Start restrictive, loosen as you gain confidence. You can always increase limits. You can't undo an overpayment.
