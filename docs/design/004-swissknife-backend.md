# Design Doc 004: SwissKnife Lightning Backend Adapter

**Status:** Implementing
**Issue:** #7
**Author:** Dario Anongba Varela

## Problem

L402sdk currently has one Lightning backend (LND via gRPC). To validate the
hexagonal architecture and provide a lower-friction option for users, we need
a second `LnBackend` implementation. SwissKnife (Numeraire's custodial Lightning
wallet) is the natural choice: it proves the adapter pattern works with a
fundamentally different backend (REST vs gRPC, custodial vs self-custodial).

## Design

### Crate: l402-swissknife

New workspace member implementing `LnBackend` for Numeraire SwissKnife's REST API.

### Architecture

```
SwissKnifeBackend
├── reqwest HTTP client
├── base_url: String (SwissKnife instance URL)
├── api_key: String (Bearer token auth)
└── LnBackend trait impl
```

### API Mapping

| LnBackend method | SwissKnife endpoint | Notes |
|------------------|-------------------|-------|
| `pay_invoice` | `POST /v1/me/payments` | Send BOLT11 as `input` field |
| `get_balance` | `GET /v1/me/balance` | Returns `available_msat`, convert to sats |
| `get_info` | `GET /v1/me` | Returns wallet ID and user info |

### Key Decisions

1. **REST + reqwest** instead of gRPC. SwissKnife exposes a REST/JSON API,
   making `reqwest` + `serde` the natural choice.

2. **Bearer token auth.** SwissKnife uses API keys via `Authorization: Bearer`
   header. Simpler than LND's TLS + macaroon scheme.

3. **No fee control.** SwissKnife manages routing internally; the `max_fee_sats`
   parameter is accepted but cannot be enforced. A `tracing::warn` is emitted
   if the actual fee exceeds the requested max after settlement.

4. **msat-to-sat conversion.** SwissKnife returns amounts in millisatoshis.
   We truncate (integer division) when converting to sats, matching Lightning
   convention.

5. **Negative balance clamping.** `available_msat` is `i64` (can be negative
   in edge cases). We clamp to 0 before conversion.

6. **Debug privacy.** `SwissKnifeBackend` has a custom `Debug` impl that
   omits the API key to prevent accidental credential leakage in logs.

7. **Environment variable config.** Supports `SWISSKNIFE_API_URL` (optional,
   defaults to `https://api.numeraire.tech`) and `SWISSKNIFE_API_KEY` (required)
   via `from_env()` constructor.

### Error Handling

```rust
pub enum SwissKnifeError {
    Http(reqwest::Error),       // Transport failures
    Api { status, message },    // Non-2xx API responses
    Auth(String),               // 401/403 specifically
    Payment(String),            // Payment-level failures
    Config(String),             // Missing configuration
}
```

All variants convert to `l402_proto::ClientError` via `From` impl:
- `Payment` → `ClientError::PaymentFailed`
- `Auth` → `ClientError::Backend` (with auth context)
- Others → `ClientError::Backend`

### Dependency Graph

```
l402-proto  ← l402-swissknife (new)
     ↑
l402-core
     ↑
l402-lnd
```

`l402-swissknife` depends only on `l402-proto` (for the `LnBackend` trait
and error types). No dependency on `l402-core` since the backend
doesn't need the L402 client engine.

## Alternatives Considered

1. **Generated OpenAPI client.** SwissKnife doesn't publish an OpenAPI spec,
   and we only need three endpoints. Hand-written types are simpler and more
   maintainable.

2. **Shared HTTP client with LND.** LND uses gRPC (tonic), SwissKnife uses
   REST (reqwest). No meaningful code to share.

## Testing Plan

- **Unit tests:** Error display, error conversion, URL trimming, Debug privacy,
  env var parsing, msat conversion, negative balance clamping.
- **Serialization tests:** JSON round-trip for all request/response types,
  unknown enum variant handling via `#[serde(other)]`.
- **Integration tests:** Will be added in a follow-up once we have a SwissKnife
  test instance or extend the mock server to simulate SwissKnife responses.

## SwissKnife vs LND Comparison

| Aspect | SwissKnife | LND |
|--------|-----------|-----|
| Model | Custodial | Self-custodial |
| Protocol | REST/JSON | gRPC/protobuf |
| Auth | API key (Bearer) | TLS cert + macaroon |
| Setup | Create account + API key | Run node + sync chain |
| Fee control | No (provider-managed) | Yes (`max_fee_sats`) |
| Use case | Quick start, SaaS | Power users, sovereignty |
