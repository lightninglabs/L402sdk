# Architecture Guide

L402sdk follows **hexagonal architecture** (ports and adapters), inspired by domain-driven design. The core protocol logic has zero external dependencies beyond standard async/HTTP libraries. Lightning backends and token stores are interchangeable through trait boundaries.

## Crate Dependency Graph

```
                         l402-proto
               (protocol types, ports, errors)
                ↑      ↑      ↑      ↑     ↑
                │      │      │      │     │
     ┌──────────┤      │      │      │     └──────────┐
     │          │      │      │      │                │
l402-lnd  L402sdk-  │  L402sdk-  L402sdk-    l402-wasm
 (gRPC+REST) swissknife│ (gRPC+REST) nwc       (WASM bindings)
     │          │      │                         wraps: lnd(rest),
     │          │      │                         cln(rest),
     │          │      │                         swissknife
     │          │      │
     └─────┬────┘      │
           │           │
     l402-core      l402-mock
     (L402 engine,     (test server)
      budget, cache)
        ↑     ↑
        │     │
   L402sdk- L402sdk-
    ffi     python
```

| Crate | Role |
|-------|------|
| `l402-proto` | Shared protocol types: `L402Challenge`, `L402Token`, `L402Error`, `ClientError`. **Also owns all port traits** (`LnBackend`, `TokenStore`) and shared domain types (`PaymentResult`, `NodeInfo`). No async runtime dependency (no tokio). WASM-safe. |
| `l402-core` | The L402 client engine. Contains `L402Client` (HTTP orchestration with reqwest), `BudgetTracker`, `InMemoryTokenStore`, and `Receipt`. Depends on `l402-proto` for port traits and shared types. |
| `l402-lnd` | Implements `LnBackend` for LND. Two feature-gated backends: `grpc` (tonic, requires tokio) and `rest` (reqwest, WASM-compatible). Depends on `l402-proto` only. |
| `l402-cln` | Implements `LnBackend` for Core Lightning (CLN). Supports gRPC with mTLS and REST with rune authentication (WASM-compatible). |
| `l402-nwc` | Implements `LnBackend` for Nostr Wallet Connect (NIP-47). |
| `l402-swissknife` | Implements `LnBackend` for Numeraire SwissKnife via REST API. Depends on `l402-proto` only. WASM-compatible. |
| `l402-mock` | A mock L402 server and mock Lightning backend for testing. No real Lightning infrastructure needed. |
| `l402-wasm` | WebAssembly bindings via `wasm-bindgen`. Exposes `WasmL402Client` (full Rust L402 engine) plus direct backend wrappers for LND REST, CLN REST, and SwissKnife. Depends on `l402-core`, `l402-proto`, and backend crates. |
| `l402-sqlite` | Persistent `TokenStore` implementation using SQLite. |
| `l402-ai-sdk` | TypeScript package providing Vercel AI SDK tools. Thin wrapper around `WasmL402Client` from `l402-wasm` — all L402 logic in Rust/WASM. |

## Ports and Adapters

The hexagonal architecture separates what the system does (core logic) from how it connects to the outside world (adapters).

### Ports (Trait Definitions)

Ports live in `l402-proto` so that adapter crates can implement them without pulling in tokio or reqwest, enabling WASM compilation:

```rust
// Lightning payment port
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait LnBackend: Send + Sync {
    async fn pay_invoice(&self, bolt11: &str, max_fee_sats: u64)
        -> Result<PaymentResult, ClientError>;
    async fn get_balance(&self) -> Result<u64, ClientError>;
    async fn get_info(&self) -> Result<NodeInfo, ClientError>;
}

// Token caching port
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait TokenStore: Send + Sync {
    async fn put(&self, endpoint: &str, macaroon: &str, preimage: &str)
        -> Result<(), ClientError>;
    async fn get(&self, endpoint: &str)
        -> Result<Option<(String, String)>, ClientError>;
    async fn remove(&self, endpoint: &str) -> Result<(), ClientError>;
    async fn clear(&self) -> Result<(), ClientError>;
}
```

The `#[cfg_attr]` conditional ensures `async_trait(?Send)` on WASM targets (where `reqwest::Response` is not `Send`) and standard `async_trait` on native targets.

### Adapters (Implementations)

Each adapter lives in its own crate:

| Port | Adapter | Crate | WASM-compatible |
|------|---------|-------|-----------------|
| `LnBackend` | LND gRPC | `l402-lnd` (feature `grpc`) | No |
| `LnBackend` | LND REST | `l402-lnd` (feature `rest`) | Yes |
| `LnBackend` | CLN gRPC | `l402-cln` | No |
| `LnBackend` | CLN REST | `l402-cln` (feature `rest`) | Yes |
| `LnBackend` | NWC (NIP-47) | `l402-nwc` | No |
| `LnBackend` | SwissKnife REST | `l402-swissknife` | Yes |
| `LnBackend` | Mock (for testing) | `l402-mock` | No |
| `TokenStore` | In-memory LRU cache | `l402-core` (built-in) | Yes |
| `TokenStore` | SQLite | `l402-sqlite` | No |

You can implement your own adapters for LDK or any other Lightning implementation. See the [Custom Backend Tutorial](tutorials/custom-backend.md).

## WASM Architecture

`l402-core` has no async runtime dependency (no tokio). It uses `std::sync::RwLock` for internal state and `web_time::Instant` for timing (transparent shim: re-exports `std::time` on native, uses `performance.now()` on WASM). This means the full L402 engine compiles to WASM.

```
l402-wasm
  ├── l402-core           (L402Client engine — no async runtime)
  ├── l402-proto          (types, ports, errors — no async runtime)
  ├── l402-lnd[rest]      (reqwest → browser fetch on WASM)
  ├── l402-cln[rest]      (reqwest → browser fetch on WASM)
  └── l402-swissknife     (reqwest → browser fetch on WASM)
```

`l402-wasm` exposes:
- **`WasmL402Client`**: Wraps the real `l402-core::L402Client` via `Rc<L402Client>`. Factory methods `withLndRest()` and `withSwissKnife()` construct the full client with Rust backends, budget tracker, and in-memory token cache. All L402 protocol logic runs in Rust.
- **`WasmLndRestBackend`** / **`WasmClnRestBackend`** / **`WasmSwissKnifeBackend`**: Direct wasm-bindgen wrappers around the Rust backends for standalone use.
- **Utility functions**: `parseL402Challenge()`, `buildL402Header()`, `version()`.

The TypeScript `l402-ai-sdk` package is a thin wrapper: it creates Vercel AI SDK tool definitions that delegate to `WasmL402Client`. No L402 protocol logic in TypeScript.

## The L402 Protocol Flow

When `L402Client.get(url)` is called, the following happens:

```
Client                     Server                     Lightning
  │                          │                           │
  │── GET /api/data ────────▶│                           │
  │                          │                           │
  │◀── 402 Payment Required ─│                           │
  │    WWW-Authenticate:     │                           │
  │    L402 macaroon="..",   │                           │
  │         invoice=".."     │                           │
  │                          │                           │
  │  [Parse L402 challenge]  │                           │
  │  [Check budget limits]   │                           │
  │                          │                           │
  │── pay_invoice(bolt11) ──────────────────────────────▶│
  │◀── PaymentResult(preimage, hash, amount) ───────────│
  │                          │                           │
  │  [Cache token]           │                           │
  │                          │                           │
  │── GET /api/data ────────▶│                           │
  │   Authorization:         │                           │
  │   L402 <macaroon>:<preimage>                         │
  │                          │                           │
  │◀── 200 OK ──────────────│                           │
  │    {"result": "..."}     │                           │
  │                          │                           │
  │  [Record receipt]        │                           │
```

On subsequent requests to the same URL, the cached token is used directly (no payment needed).

## Budget System

The `BudgetTracker` enforces spending limits at multiple granularities:

- **Per-request**: Maximum satoshis for a single payment
- **Hourly**: Rolling hourly cap
- **Daily**: Rolling daily cap
- **Total**: Lifetime cap for the client instance
- **Domain-specific**: Override budgets for specific API domains

Budget checks happen before payment. If a limit would be exceeded, `ClientError::BudgetExceeded` is returned and no payment is attempted.

## Design Principles

1. **WASM-safe foundation.** Both `l402-proto` and `l402-core` have zero async runtime dependency. The full L402 engine compiles to WASM. Backend crates that use reqwest get browser `fetch` for free on WASM targets.

2. **Zero-dependency core.** `l402-core` depends only on `l402-proto`, reqwest, and `web-time`. No async runtime, no Lightning-specific dependencies leak into the core. Compiles to WASM.

3. **Swap anything.** Need a different Lightning backend? Implement `LnBackend`. Need persistent token storage? Implement `TokenStore`. The core doesn't care.

4. **Test without infrastructure.** `l402-mock` provides a complete L402 server and mock Lightning backend. `l402-wasm` includes an in-process mock for browser testing. No real Lightning node needed.

5. **Receipts by default.** Every payment is recorded as a `Receipt` with amount, fees, latency, and payment hash. This makes cost analysis and auditing trivial.

6. **Safety first for agents.** AI agents spending real money need guardrails. The budget system is not optional decoration; it's a first-class concern built into the protocol flow.
