# Design: NWC (Nostr Wallet Connect) Lightning Backend

**Issue:** #50
**Author:** Dario Anongba Varela
**Date:** 2026-03-21

## Problem

bolt402 currently supports two Lightning backends: LND (gRPC) and SwissKnife (REST). Both require direct access to a Lightning node or service. Many developers and AI agents don't run their own nodes — they use hosted wallets that support NWC (Nostr Wallet Connect, NIP-47).

Adding an NWC backend lowers the barrier to entry dramatically: users just need a `nostr+walletconnect://` connection string from any compatible wallet (Alby Hub, Mutiny, LNbits, Phoenixd, etc.).

## Proposed Design

### New Crate: `bolt402-nwc`

```
crates/bolt402-nwc/
├── Cargo.toml
└── src/
    ├── lib.rs        # Module root, re-exports
    ├── backend.rs    # NwcBackend implementing LnBackend
    ├── error.rs      # NwcError type
    └── uri.rs        # NWC URI parsing
```

### Dependency Graph

```
bolt402-proto  (no internal deps)
     ↑
bolt402-core   (depends on proto)
     ↑
bolt402-nwc    (depends on core: implements LnBackend via NIP-47)
```

### API Sketch

```rust
use bolt402_nwc::NwcBackend;

// From a NWC connection URI
let backend = NwcBackend::new("nostr+walletconnect://...").await?;

// From environment variable (NWC_CONNECTION_URI)
let backend = NwcBackend::from_env().await?;

// Use with L402Client
let client = L402Client::builder()
    .ln_backend(backend)
    .token_store(InMemoryTokenStore::default())
    .budget(Budget::unlimited())
    .build()?;
```

### NIP-47 Protocol Mapping

| NIP-47 Method | LnBackend Method | Notes |
|---|---|---|
| `pay_invoice` | `pay_invoice()` | Sends BOLT11, receives preimage |
| `get_balance` | `get_balance()` | Returns available balance in sats |
| `get_info` | `get_info()` | Returns node pubkey, alias, channels |

### NWC URI Format

```
nostr+walletconnect://<wallet_pubkey>?relay=<relay_url>&secret=<app_secret_key>[&lud16=<lud16>]
```

The URI contains:
- **wallet_pubkey**: The wallet service's Nostr pubkey
- **relay**: Nostr relay URL(s) for communication
- **secret**: App-side secret key for encryption (NIP-04/NIP-44)

### Communication Flow

```
Agent (bolt402-nwc)                     Nostr Relay                     Wallet
     │                                       │                            │
     │ ─── Encrypted NIP-47 request ────────>│                            │
     │     (kind: 23194)                     │ ─── Forward ──────────────>│
     │                                       │                            │
     │                                       │<─── Encrypted response ────│
     │<─── Forward ──────────────────────────│     (kind: 23195)          │
     │                                       │                            │
```

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum NwcError {
    #[error("invalid NWC URI: {0}")]
    InvalidUri(String),

    #[error("relay connection failed: {0}")]
    RelayConnection(String),

    #[error("NWC request timed out after {timeout_secs}s")]
    Timeout { timeout_secs: u64 },

    #[error("wallet returned error: {code} - {message}")]
    WalletError { code: String, message: String },

    #[error("failed to decrypt response: {0}")]
    Decryption(String),

    #[error("payment failed: {0}")]
    Payment(String),
}
```

### Key Decisions

1. **Use `nostr-sdk` crate**: Mature, well-maintained, already handles NIP-04/NIP-44 encryption, relay management, and event construction. Avoids reimplementing Nostr transport.

2. **Timeout handling**: NWC communication is asynchronous via relays. Default timeout of 60 seconds for `pay_invoice`, 15 seconds for `get_balance`/`get_info`. Configurable via builder.

3. **Connection URI parsing**: Custom parser rather than depending on `url` crate — the `nostr+walletconnect://` scheme isn't a standard URL and needs specific handling.

4. **No relay persistence**: The backend connects to relays on creation and maintains the connection for the lifetime of the object. No persistent relay subscriptions beyond the session.

5. **NIP-44 preferred**: Use NIP-44 encryption when possible (newer, more secure), with NIP-04 fallback for older wallets.

### Alternatives Considered

- **Raw WebSocket client**: Lower dependency count but would require reimplementing Nostr event creation, encryption, and relay management. Not worth it.
- **`nostr` crate only (without `nostr-sdk`)**: Lighter but lacks relay pool management and subscription handling. `nostr-sdk` is the standard choice.

### Testing Plan

1. **Unit tests**: URI parsing (valid/invalid URIs), error type conversions
2. **Integration tests**: Use a mock relay or test against a local NWC-compatible service
3. **Doc tests**: Usage examples in documentation

### Workspace Changes

- Add `bolt402-nwc` to `Cargo.toml` workspace members and default-members
- Add `nostr-sdk` to workspace dependencies
- Update README.md package table
- Update AGENTS.md architecture diagram
