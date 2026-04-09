# Design Doc 002: Mock L402 Server

**Status:** Proposed
**Issue:** #3
**Author:** Dario Anongba Varela

## Problem

We need a way to test the L402 flow end-to-end without real Lightning infrastructure.
The mock server simulates an L402-protected API: it returns 402 challenges with valid
macaroons and fake invoices, then validates authorization tokens.

This is critical for:
- Unit/integration tests for L402Client
- Testing LND and SwissKnife backend adapters
- Demo binary showing the full flow
- CI (no Lightning node required)

## Design

### Crate: bolt402-mock

Lightweight HTTP server built on axum. Depends only on bolt402-proto for shared types.

### Components

```
MockL402Server
├── axum Router
├── HashMap<String, EndpointConfig>  (protected endpoints)
└── Arc<RwLock<HashMap<String, PendingChallenge>>>  (issued challenges)

PendingChallenge
├── preimage (32 random bytes, hex)
├── payment_hash (SHA256 of preimage, hex)
├── macaroon (base64-encoded test macaroon)
└── invoice (fake BOLT11 string)
```

### Flow

1. Client hits a protected endpoint without auth
2. Server generates PendingChallenge (random preimage, hash it, create macaroon)
3. Server responds 402 with `WWW-Authenticate: L402 macaroon="...", invoice="..."`
4. Client "pays" by extracting the preimage from the mock (or via a mock LnBackend that knows all preimages)
5. Client retries with `Authorization: L402 <macaroon>:<preimage>`
6. Server validates: SHA256(preimage) == payment_hash AND macaroon matches
7. Server responds 200 with the resource

### Mock LnBackend

A `MockLnBackend` that implements `LnBackend` and always "pays" successfully by
returning the preimage. It communicates with the mock server via a shared preimage
registry (Arc), so the "payment" is just looking up the preimage for the invoice.

### API

```rust
// Server setup
let server = MockL402Server::builder()
    .endpoint("/api/data", EndpointConfig::new(100))   // 100 sats
    .endpoint("/api/premium", EndpointConfig::new(500)) // 500 sats
    .build()
    .await?;

// Get a mock backend that "pays" by looking up preimages
let backend = server.mock_backend();

// Use with L402Client
let client = L402Client::builder()
    .backend(backend)
    .build()?;

let response = client.get(&format!("{}/api/data", server.url())).await?;
```

### Key Decisions

- **axum** for the HTTP server (lightweight, async, widely used in Rust ecosystem)
- **Shared preimage registry** between server and mock backend (simple, no real network needed)
- **Real SHA256 validation** so the crypto flow is exercised even in tests
- **Configurable per-endpoint** pricing and behavior (error injection for testing edge cases)

## Testing Plan

- Challenge generation and validation (unit tests in challenge.rs)
- Server responds 402 for protected endpoints (integration test)
- Server accepts valid L402 token (integration test)
- Server rejects invalid preimage (integration test)
- Server rejects invalid macaroon (integration test)
- Full L402Client flow against mock server (integration test)
- Budget enforcement with mock server (integration test)
