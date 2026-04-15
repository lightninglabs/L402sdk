//! # L402sdk Budget Control Example
//!
//! Demonstrates budget configuration and receipt-based cost tracking.
//!
//! Run with:
//! ```bash
//! cargo run -p example-budget-control
//! ```
//!
//! Note: Full per-request budget enforcement requires BOLT11 invoice amount
//! decoding, which is planned but not yet implemented. Currently the budget
//! configuration is validated and the receipt system tracks actual costs
//! accurately.

use l402_core::budget::Budget;
use l402_core::cache::InMemoryTokenStore;
use l402_core::{L402Client, L402ClientConfig};
use l402_mock::{EndpointConfig, MockL402Server};

#[tokio::main]
async fn main() {
    println!("L402sdk — Budget Control Example");
    println!("=================================");
    println!();

    // Start a mock server with endpoints at different prices
    let server = MockL402Server::builder()
        .endpoint(
            "/api/basic",
            EndpointConfig::new(50).with_body(r#"{"tier":"basic"}"#),
        )
        .endpoint(
            "/api/standard",
            EndpointConfig::new(200).with_body(r#"{"tier":"standard"}"#),
        )
        .endpoint(
            "/api/premium",
            EndpointConfig::new(500).with_body(r#"{"tier":"premium"}"#),
        )
        .build()
        .await
        .expect("failed to start mock server");

    println!("Mock server at {}", server.url());
    println!();

    // Configure a budget
    // When BOLT11 amount decoding is added, these limits will be enforced
    // before payment. Currently, receipts track actual costs accurately.
    let budget = Budget {
        per_request_max: Some(1000),
        hourly_max: None,
        daily_max: Some(10_000),
        total_max: None,
        domain_budgets: Default::default(),
    };

    println!("Budget configured: max 1,000 sats/request, max 10,000 sats/day");
    println!();

    let client = L402Client::builder()
        .ln_backend(server.mock_backend())
        .token_store(InMemoryTokenStore::new(100))
        .budget(budget)
        .config(L402ClientConfig {
            max_fee_sats: 100,
            max_retries: 1,
            user_agent: "L402sdk-budget-example/0.1".to_string(),
        })
        .build()
        .expect("failed to build client");

    // Make several requests at different price points
    let endpoints = [
        ("/api/basic", "Basic (50 sats)"),
        ("/api/standard", "Standard (200 sats)"),
        ("/api/premium", "Premium (500 sats)"),
        ("/api/basic", "Basic again (cached)"),
    ];

    for (i, (path, label)) in endpoints.iter().enumerate() {
        let url = format!("{}{path}", server.url());
        println!("[{}] {label}", i + 1);

        let response = client.get(&url).await.expect("request failed");
        let status = response.status();
        let receipt = response.receipt().cloned();

        let body = response.text().await.unwrap();

        if let Some(r) = receipt {
            println!(
                "    ✓ {status} | Paid {} sats ({}ms) | {body}",
                r.total_cost_sats(),
                r.latency_ms,
            );
        } else {
            println!("    ✓ {status} | Cached (no payment) | {body}");
        }
    }

    println!();

    // Receipt-based cost analysis
    println!("Cost Analysis");
    println!("=============");

    let receipts = client.receipts().await;

    let total: u64 = receipts.iter().map(|r| r.total_cost_sats()).sum();
    let avg_latency =
        receipts.iter().map(|r| r.latency_ms).sum::<u64>() / receipts.len().max(1) as u64;

    println!("  Payments made: {}", receipts.len());
    println!("  Total spent: {total} sats");
    println!("  Average latency: {avg_latency}ms");
    println!();

    println!("  Per-endpoint breakdown:");
    for receipt in &receipts {
        println!(
            "    {} → {} sats (status {}, {}ms)",
            receipt.endpoint,
            receipt.total_cost_sats(),
            receipt.response_status,
            receipt.latency_ms,
        );
    }

    if let Some(max) = receipts.iter().max_by_key(|r| r.total_cost_sats()) {
        println!();
        println!(
            "  Most expensive: {} ({} sats)",
            max.endpoint,
            max.total_cost_sats()
        );
    }
}
