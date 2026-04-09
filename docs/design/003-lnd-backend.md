# Design Doc 003: LND gRPC Backend Adapter

**Status:** Implemented
**Issue:** #4
**Author:** Dario Anongba Varela

## Problem

bolt402 needs a real Lightning backend to make actual payments. LND is the most
widely deployed Lightning implementation and the one used by Lightning Labs
(Dario's employer). It's the natural first backend.

## Design

### Crate: bolt402-lnd

Implements `LnBackend` from `bolt402-proto` using LND's gRPC API, and now also ships a feature-gated REST adapter for WASM/browser environments.

### Architecture

```
LndGrpcBackend
├── tonic gRPC channel (TLS + macaroon auth)
├── LND endpoint URL
├── TLS certificate
└── Admin macaroon (or invoice macaroon for read-only)
```

### LnBackend Implementation

```rust
#[async_trait]
impl LnBackend for LndGrpcBackend {
    async fn pay_invoice(&self, invoice: &str, max_fee_sats: u64)
        -> Result<PaymentResult, ClientError>;

    async fn get_balance(&self) -> Result<u64, ClientError>;

    async fn get_info(&self) -> Result<NodeInfo, ClientError>;
}
```

- `pay_invoice`: Calls `routerrpc.Router/SendPaymentV2` with the BOLT11 invoice.
  Respects `max_fee_sats` via `fee_limit_sat` and returns preimage plus payment hash on success.
- `get_balance`: Calls `lnrpc.Lightning/ChannelBalance`, returns local balance in sats.
- `get_info`: Calls `lnrpc.Lightning/GetInfo`, returns alias and pubkey.

### gRPC Proto Generation

Two options:
1. **tonic-build at compile time** from .proto files (standard but needs protoc)
2. **Pre-generated code** checked into the repo (no build dependency)
3. **Use tonic-lnd crate** if one exists and is maintained

Decision: vendor the required `.proto` files in-repo and generate code at build time.

### Configuration

```rust
let backend = LndGrpcBackend::connect(
    "https://localhost:10009",
    "/path/to/tls.cert",
    "/path/to/admin.macaroon",
).await?;
```

Also support environment variables:
- `LND_GRPC_HOST`
- `LND_TLS_CERT_PATH`
- `LND_MACAROON_PATH`

### SwissKnife Backend (future, #next)

After LND, we add a SwissKnife backend that talks to Numeraire SwissKnife's API.
Same `LnBackend` trait, different adapter. This proves the hexagonal architecture
works with multiple backends.

## Testing Plan

- Unit tests with mocked gRPC responses
- Integration tests against LND regtest (Docker Compose with bitcoind + lnd)
- Test: successful payment, payment failure, timeout, fee limit exceeded
- The mock server (issue #3) provides the L402 endpoint to test against
