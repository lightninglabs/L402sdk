# Implementing a Custom Lightning Backend

L402sdk's hexagonal architecture makes it straightforward to add support for any Lightning implementation. This tutorial walks through implementing the `LnBackend` trait.

## The LnBackend Trait

The `LnBackend` trait is the port that defines how L402sdk pays invoices:

```rust
use async_trait::async_trait;
use l402_proto::port::{LnBackend, PaymentResult, NodeInfo};
use l402_proto::ClientError;

#[async_trait]
pub trait LnBackend: Send + Sync {
    /// Pay a BOLT11 Lightning invoice.
    async fn pay_invoice(
        &self,
        bolt11: &str,
        max_fee_sats: u64,
    ) -> Result<PaymentResult, ClientError>;

    /// Get the current spendable balance in satoshis.
    async fn get_balance(&self) -> Result<u64, ClientError>;

    /// Get information about the connected Lightning node.
    async fn get_info(&self) -> Result<NodeInfo, ClientError>;
}
```

The three methods serve different purposes:
- `pay_invoice` is the core: it pays a BOLT11 invoice and returns proof of payment
- `get_balance` lets the client or surrounding application check available funds
- `get_info` provides node metadata (used for diagnostics and the AI SDK balance tool)

## Example: Core Lightning (CLN) Backend

Let's implement a backend for [Core Lightning](https://github.com/ElementsProject/lightning) using its JSON-RPC interface.

### Step 1: Create the Crate

```bash
mkdir crates/l402-cln
```

`crates/l402-cln/Cargo.toml`:
```toml
[package]
name = "l402-cln"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
l402-proto = { workspace = true }
async-trait = { workspace = true }
reqwest = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
```

### Step 2: Define the Backend Struct

```rust
use l402_proto::port::{LnBackend, PaymentResult, NodeInfo};
use l402_proto::ClientError;
use async_trait::async_trait;

/// Core Lightning backend using the JSON-RPC interface.
pub struct ClnBackend {
    rpc_url: String,
    client: reqwest::Client,
}

impl ClnBackend {
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            client: reqwest::Client::new(),
        }
    }
}
```

### Step 3: Implement pay_invoice

This is the critical method. It must:
1. Send the BOLT11 invoice to the Lightning node
2. Wait for the payment to complete
3. Return the preimage (proof of payment), payment hash, amount, and fee

```rust
#[async_trait]
impl LnBackend for ClnBackend {
    async fn pay_invoice(
        &self,
        bolt11: &str,
        max_fee_sats: u64,
    ) -> Result<PaymentResult, ClientError> {
        // CLN JSON-RPC: method "pay"
        let response = self.client
            .post(&self.rpc_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "pay",
                "params": {
                    "bolt11": bolt11,
                    "maxfeepercent": 0.5,
                    "maxfee": max_fee_sats * 1000, // CLN uses millisatoshis
                }
            }))
            .send()
            .await
            .map_err(|e| ClientError::Backend {
                reason: format!("CLN RPC failed: {e}"),
            })?;

        let body: serde_json::Value = response.json().await
            .map_err(|e| ClientError::Backend {
                reason: format!("invalid CLN response: {e}"),
            })?;

        // Extract fields from CLN response
        let result = body.get("result").ok_or(ClientError::PaymentFailed {
            reason: "CLN returned no result".to_string(),
        })?;

        Ok(PaymentResult {
            preimage: result["payment_preimage"]
                .as_str().unwrap_or_default().to_string(),
            payment_hash: result["payment_hash"]
                .as_str().unwrap_or_default().to_string(),
            amount_sats: result["amount_sent_msat"]
                .as_u64().unwrap_or(0) / 1000,
            fee_sats: (result["amount_sent_msat"].as_u64().unwrap_or(0)
                - result["amount_msat"].as_u64().unwrap_or(0)) / 1000,
        })
    }

    async fn get_balance(&self) -> Result<u64, ClientError> {
        let response = self.client
            .post(&self.rpc_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "listfunds",
                "params": {}
            }))
            .send()
            .await
            .map_err(|e| ClientError::Backend {
                reason: format!("CLN RPC failed: {e}"),
            })?;

        let body: serde_json::Value = response.json().await
            .map_err(|e| ClientError::Backend {
                reason: format!("invalid CLN response: {e}"),
            })?;

        // Sum channel balances
        let channels = body["result"]["channels"].as_array();
        let balance_msat: u64 = channels
            .map(|chs| chs.iter()
                .filter_map(|ch| ch["our_amount_msat"].as_u64())
                .sum())
            .unwrap_or(0);

        Ok(balance_msat / 1000)
    }

    async fn get_info(&self) -> Result<NodeInfo, ClientError> {
        let response = self.client
            .post(&self.rpc_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "getinfo",
                "params": {}
            }))
            .send()
            .await
            .map_err(|e| ClientError::Backend {
                reason: format!("CLN RPC failed: {e}"),
            })?;

        let body: serde_json::Value = response.json().await
            .map_err(|e| ClientError::Backend {
                reason: format!("invalid CLN response: {e}"),
            })?;

        let result = &body["result"];

        Ok(NodeInfo {
            pubkey: result["id"].as_str().unwrap_or_default().to_string(),
            alias: result["alias"].as_str().unwrap_or_default().to_string(),
            num_active_channels: result["num_active_channels"]
                .as_u64().unwrap_or(0) as u32,
        })
    }
}
```

### Step 4: Use Your Backend

```rust
use l402_core::L402Client;
use l402_core::budget::Budget;
use l402_core::cache::InMemoryTokenStore;

let backend = ClnBackend::new("http://localhost:9835");

let client = L402Client::builder()
    .ln_backend(backend)
    .token_store(InMemoryTokenStore::default())
    .budget(Budget {
        per_request_max: Some(1000),
        daily_max: Some(50_000),
        ..Budget::unlimited()
    })
    .build()
    .unwrap();

let response = client.get("https://api.example.com/data").await?;
```

### Step 5: Test with l402-mock

You don't need a real CLN node to test your client integration. Use `l402-mock`:

```rust
#[tokio::test]
async fn test_cln_client_integration() {
    let server = MockL402Server::builder()
        .endpoint("/test", EndpointConfig::new(50))
        .build()
        .await
        .unwrap();

    // Use the mock backend instead of ClnBackend for tests
    let client = L402Client::builder()
        .ln_backend(server.mock_backend())
        .token_store(InMemoryTokenStore::default())
        .budget(Budget::unlimited())
        .build()
        .unwrap();

    let url = format!("{}/test", server.url());
    let resp = client.get(&url).await.unwrap();
    assert!(resp.paid());
    assert_eq!(resp.status(), 200);
}
```

## TypeScript / WASM Note

`l402-ai-sdk` no longer exposes a native TypeScript `LnBackend` interface. The package is a thin wrapper around `l402-wasm`, which in turn wraps Rust backends and the Rust `L402Client`.

If you need a new backend in JavaScript or TypeScript:

- implement the adapter in Rust by adding a new crate (or extending an existing backend crate) that implements `LnBackend`
- expose it through `crates/l402-wasm` with a wasm-bindgen wrapper, following `WasmLndRestBackend`, `WasmClnRestBackend`, or `WasmSwissKnifeBackend`
- add a `WasmL402Client` constructor if the backend should support the full end-to-end L402 flow from JS/TS

## Checklist for New Backends

- [ ] Implement all three methods of `LnBackend`
- [ ] Map node-specific errors to `ClientError` variants
- [ ] Handle both successful and failed payments gracefully
- [ ] Return accurate `fee_sats` (not zero) for proper receipt tracking
- [ ] Add unit tests with mocked HTTP responses
- [ ] Add integration tests using `l402-mock` for the L402Client flow
- [ ] Add a WASM wrapper in `crates/l402-wasm` if the backend should be available from JS/TS
- [ ] Document connection parameters and authentication requirements
