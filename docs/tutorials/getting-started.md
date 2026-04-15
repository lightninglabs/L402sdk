# Getting Started with L402sdk

This tutorial walks you through the full L402 payment flow using `l402-mock`. No real Lightning node needed.

## Prerequisites

- Rust 1.85+ (see [rustup.rs](https://rustup.rs))
- Cargo (comes with Rust)

## Setup

Create a new project:

```bash
cargo init my-l402-app
cd my-l402-app
```

Add dependencies to `Cargo.toml`:

```toml
[dependencies]
l402-core = { git = "https://github.com/lightninglabs/L402sdk" }
l402-mock = { git = "https://github.com/lightninglabs/L402sdk" }
tokio = { version = "1", features = ["full"] }
```

## Step 1: Start a Mock L402 Server

The mock server simulates an API that requires Lightning payments. You configure which endpoints are protected and how much they cost:

```rust
use l402_mock::{MockL402Server, EndpointConfig};

#[tokio::main]
async fn main() {
    // Create a mock server with a protected endpoint
    let server = MockL402Server::builder()
        .endpoint(
            "/api/data",
            EndpointConfig::new(100) // Costs 100 satoshis
                .with_body(r#"{"result": "Here is your premium data!"}"#),
        )
        .build()
        .await
        .expect("failed to start mock server");

    println!("Mock server running at {}", server.url());
}
```

The server:
- Returns `402 Payment Required` with an L402 challenge for protected endpoints
- Accepts `Authorization: L402 <macaroon>:<preimage>` headers as proof of payment
- Returns `404` for unconfigured paths

## Step 2: Create an L402 Client

The `L402Client` handles the full payment flow automatically. It needs:
- A **Lightning backend** (we'll use the mock one)
- A **token store** (in-memory cache)
- A **budget** (spending limits)

```rust
use l402_core::L402Client;
use l402_core::budget::Budget;
use l402_core::cache::InMemoryTokenStore;

// The mock backend "pays" invoices by looking up preimages
// from the mock server's registry (no real money involved)
let backend = server.mock_backend();

let client = L402Client::builder()
    .ln_backend(backend)
    .token_store(InMemoryTokenStore::new(100))
    .budget(Budget::unlimited())
    .build()
    .expect("failed to build client");
```

## Step 3: Make a Request

When you call `client.get()`, the L402 flow happens automatically:

```rust
let url = format!("{}/api/data", server.url());
let response = client.get(&url).await.expect("request failed");

println!("Status: {}", response.status());   // 200
println!("Paid: {}", response.paid());       // true (first request)

if let Some(receipt) = response.receipt() {
    println!("Amount: {} sats", receipt.amount_sats);
    println!("Fee: {} sats", receipt.fee_sats);
    println!("Payment hash: {}", receipt.payment_hash);
}

let body = response.text().await.unwrap();
println!("Body: {body}");
```

Behind the scenes:
1. Client sends `GET /api/data`
2. Server returns `402` with `WWW-Authenticate: L402 macaroon="...", invoice="..."`
3. Client parses the challenge
4. Client pays the Lightning invoice via the backend
5. Client caches the resulting token (macaroon + preimage)
6. Client retries the request with `Authorization: L402 <macaroon>:<preimage>`
7. Server validates the token and returns `200` with the data

## Step 4: Observe Token Caching

Make the same request again:

```rust
let response = client.get(&url).await.expect("request failed");

println!("Paid: {}", response.paid()); // false (token was cached!)
```

The client reused the cached token. No second payment was needed.

## Step 5: Check Receipts

Every payment is recorded as a receipt:

```rust
let receipts = client.receipts().await;

for receipt in &receipts {
    println!(
        "{} → {} sats ({}ms)",
        receipt.endpoint,
        receipt.total_cost_sats(),
        receipt.latency_ms,
    );
}

let total_spent = client.total_spent().await;
println!("Total spent: {total_spent} sats");
```

## Full Example

See [`examples/basic-mock/main.rs`](../../examples/basic-mock/main.rs) for a complete, runnable version of this tutorial.

## Next Steps

- [Budget Control](budget-control.md) — Set spending limits for autonomous agents
- [Custom Backend](custom-backend.md) — Implement `LnBackend` for your Lightning node
- [Architecture Guide](../architecture.md) — Understand the hexagonal design
