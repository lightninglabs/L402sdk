# Design Doc 001: L402Client Engine

**Status:** Implemented (PR #6, merged 2026-03-14)
**Issue:** #2
**Author:** Toshi

> Note: This design doc was written retroactively. Future features will have
> design docs reviewed before implementation begins.

## Problem

bolt402 needs a central client that transparently handles L402 authentication.
When an HTTP request returns 402 Payment Required, the client should
automatically parse the challenge, pay the invoice, cache the token, and retry.

## Design

### Architecture

```
L402Client
├── reqwest::Client (HTTP)
├── Arc<dyn LnBackend> (Lightning payments)
├── Arc<dyn TokenStore> (Token cache)
├── BudgetTracker (Spending limits)
└── Vec<Receipt> (Payment audit log)
```

### Flow

1. Client sends HTTP request
2. If response is 402 with `WWW-Authenticate: L402` header:
   a. Parse challenge via `L402Challenge::from_header`
   b. Check token cache (skip payment if cached)
   c. Check budget (reject if over limit)
   d. Pay invoice via `LnBackend::pay_invoice`
   e. Construct `L402Token` from macaroon + preimage
   f. Cache token via `TokenStore::put`
   g. Retry request with `Authorization: L402` header
   h. Record `Receipt`
3. Any other response: pass through

### API

Builder pattern for configuration:

```rust
let client = L402Client::builder()
    .backend(lnd_backend)
    .token_store(InMemoryTokenStore::new(1000))
    .budget(Budget::new(Some(1000), Some(10_000), Some(100_000), None))
    .max_retries(3)
    .build()?;

let response = client.get("https://api.example.com/resource").await?;
```

### Key Decisions

- **Wraps reqwest::Client** internally (not generic over HTTP client). Keeps API simple.
- **Builder pattern** for ergonomic configuration with sensible defaults.
- **Budget checked before payment**, not after. Fail fast.
- **Token cache keyed by endpoint URL.** Simple but effective for most use cases.
- **Thread-safe via Arc** on backend, store, and budget tracker.

## Alternatives Considered

- **Generic over HTTP client:** More flexible but adds type parameter noise. reqwest covers 95% of use cases. Can add later if needed.
- **Middleware/tower layer:** Would compose better with existing HTTP stacks but harder to use standalone. Could add a tower layer later that wraps L402Client internally.

## Testing

- Unit tests with mock LnBackend (mockall)
- Tests for: happy path, cached token reuse, budget exceeded, payment failure, missing challenge, retry logic
