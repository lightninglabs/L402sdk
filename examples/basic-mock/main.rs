//! # L402sdk Getting Started Example
//!
//! Demonstrates the full L402 payment flow using the mock server.
//! No real Lightning node needed.
//!
//! Run with:
//! ```bash
//! cargo run --example basic-mock
//! ```

use l402_core::budget::Budget;
use l402_core::cache::InMemoryTokenStore;
use l402_core::{L402Client, L402ClientConfig};
use l402_mock::{EndpointConfig, MockL402Server};

#[tokio::main]
async fn main() {
    println!("L402sdk — Getting Started Example");
    println!("==================================");
    println!();

    // Step 1: Start a mock L402 server with protected endpoints
    let server = MockL402Server::builder()
        .endpoint(
            "/api/data",
            EndpointConfig::new(100).with_body(r#"{"result":"Here is your premium data!"}"#),
        )
        .endpoint(
            "/api/premium",
            EndpointConfig::new(500).with_body(r#"{"result":"Exclusive premium content."}"#),
        )
        .build()
        .await
        .expect("failed to start mock server");

    println!("Mock server running at {}", server.url());
    println!();

    // Step 2: Create an L402 client with the mock Lightning backend
    let client = L402Client::builder()
        .ln_backend(server.mock_backend())
        .token_store(InMemoryTokenStore::new(100))
        .budget(Budget::unlimited())
        .config(L402ClientConfig {
            max_fee_sats: 1000,
            max_retries: 1,
            user_agent: "L402sdk-example/0.1".to_string(),
        })
        .build()
        .expect("failed to build client");

    // Step 3: First request — triggers L402 payment
    let url = format!("{}/api/data", server.url());
    println!("[1] GET {url}");
    println!("    Expected: 402 → pay 100 sats → 200");

    let response = client.get(&url).await.expect("request failed");
    println!("    Status: {}", response.status());
    println!("    Paid: {}", response.paid());

    if let Some(receipt) = response.receipt() {
        println!(
            "    Amount: {} sats + {} fee",
            receipt.amount_sats, receipt.fee_sats
        );
    }

    let body = response.text().await.unwrap();
    println!("    Body: {body}");
    println!();

    // Step 4: Same request — uses cached token (no payment)
    println!("[2] GET {url} (same endpoint)");
    println!("    Expected: cached token → 200 (no payment)");

    let response = client.get(&url).await.expect("request failed");
    println!("    Status: {}", response.status());
    println!("    Paid: {}", response.paid());

    let body = response.text().await.unwrap();
    println!("    Body: {body}");
    println!();

    // Step 5: Different endpoint — new payment required
    let url_premium = format!("{}/api/premium", server.url());
    println!("[3] GET {url_premium}");
    println!("    Expected: 402 → pay 500 sats → 200");

    let response = client.get(&url_premium).await.expect("request failed");
    println!("    Status: {}", response.status());
    println!("    Paid: {}", response.paid());

    if let Some(receipt) = response.receipt() {
        println!(
            "    Amount: {} sats + {} fee",
            receipt.amount_sats, receipt.fee_sats
        );
    }

    let body = response.text().await.unwrap();
    println!("    Body: {body}");
    println!();

    // Step 6: Receipt summary
    println!("Receipt Summary");
    println!("===============");

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

    let total: u64 = receipts.iter().map(|r| r.total_cost_sats()).sum();
    println!();
    println!("  Payments: {}", receipts.len());
    println!("  Total: {total} sats");
}
