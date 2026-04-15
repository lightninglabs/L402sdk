//! Interactive demo of the L402sdk L402 client.
//!
//! Run with: `cargo run --example demo`
//!
//! This demo:
//! 1. Starts a mock L402 server with two protected endpoints
//! 2. Creates an L402 client with a mock Lightning backend
//! 3. Makes a request, showing the full 402 → pay → retry → 200 flow
//! 4. Makes a second request to demonstrate token caching
//! 5. Accesses a different endpoint to show separate payments
//! 6. Prints a receipt summary

use l402_core::budget::Budget;
use l402_core::cache::InMemoryTokenStore;
use l402_core::{L402Client, L402ClientConfig};
use l402_mock::{EndpointConfig, MockL402Server};

#[tokio::main]
async fn main() {
    println!("╔══════════════════════════════════════════════════╗");
    println!("║           L402sdk — L402 Client Demo            ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();

    // --- Step 1: Start mock server ---
    println!("▸ Starting mock L402 server...");
    let server = MockL402Server::builder()
        .endpoint(
            "/api/data",
            EndpointConfig::new(100).with_body(r#"{"result":"Here is your premium data!"}"#),
        )
        .endpoint(
            "/api/premium",
            EndpointConfig::new(500)
                .with_body(r#"{"result":"This is the exclusive premium content."}"#),
        )
        .build()
        .await
        .expect("failed to start mock server");

    println!("  ✓ Mock server running at {}", server.url());
    println!();

    // --- Step 2: Create L402 client ---
    println!("▸ Creating L402 client with mock Lightning backend...");
    let backend = server.mock_backend();
    let store = InMemoryTokenStore::new(1024);

    let client = L402Client::builder()
        .ln_backend(backend)
        .token_store(store)
        .budget(Budget::unlimited())
        .config(L402ClientConfig {
            max_fee_sats: 1000,
            max_retries: 1,
            user_agent: "L402sdk-demo/0.1".to_string(),
        })
        .build()
        .expect("failed to build client");

    println!("  ✓ Client ready (budget: unlimited)");
    println!();

    // --- Step 3: First request (triggers payment) ---
    let url_data = format!("{}/api/data", server.url());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("▸ Request 1: GET {url_data}");
    println!("  Expecting: 402 → pay invoice → retry with token → 200");
    println!();

    let response = client.get(&url_data).await.expect("request failed");
    println!("  Status: {}", response.status());
    println!("  Paid:   {}", response.paid());

    if let Some(receipt) = response.receipt() {
        println!("  Amount: {} sats", receipt.amount_sats);
        println!("  Fee:    {} sats", receipt.fee_sats);
        println!("  Hash:   {}…", &receipt.payment_hash[..16]);
    }

    let body = response.text().await.unwrap();
    println!("  Body:   {body}");
    println!();

    // --- Step 4: Second request (cached token) ---
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("▸ Request 2: GET {url_data} (same endpoint)");
    println!("  Expecting: cached token → 200 (no payment)");
    println!();

    let response = client.get(&url_data).await.expect("request failed");
    println!("  Status: {}", response.status());
    println!("  Paid:   {} (token was cached!)", response.paid());
    let body = response.text().await.unwrap();
    println!("  Body:   {body}");
    println!();

    // --- Step 5: Different endpoint (new payment) ---
    let url_premium = format!("{}/api/premium", server.url());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("▸ Request 3: GET {url_premium}");
    println!("  Expecting: new 402 → pay (500 sats) → 200");
    println!();

    let response = client.get(&url_premium).await.expect("request failed");
    println!("  Status: {}", response.status());
    println!("  Paid:   {}", response.paid());

    if let Some(receipt) = response.receipt() {
        println!("  Amount: {} sats", receipt.amount_sats);
    }

    let body = response.text().await.unwrap();
    println!("  Body:   {body}");
    println!();

    // --- Step 6: Receipt summary ---
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("▸ Receipt Summary");
    println!();

    let receipts = client.receipts().await;
    for (i, receipt) in receipts.iter().enumerate() {
        println!(
            "  #{}: {} → {} sats (status {}, {}ms)",
            i + 1,
            receipt.endpoint,
            receipt.total_cost_sats(),
            receipt.response_status,
            receipt.latency_ms,
        );
    }

    let total: u64 = receipts
        .iter()
        .map(l402_core::receipt::Receipt::total_cost_sats)
        .sum();
    println!();
    println!("  Total payments: {}", receipts.len());
    println!("  Total spent:    {total} sats");
    println!();
    println!("Done.");
}
