//! Integration tests for the L402sdk L402 client.
//!
//! These tests spin up a [`MockL402Server`] and wire it to an [`L402Client`]
//! via [`MockLnBackend`], exercising the full HTTP-based L402 flow over
//! localhost without any real Lightning infrastructure.

use l402_core::budget::Budget;
use l402_core::cache::InMemoryTokenStore;
use l402_core::{L402Client, L402ClientConfig};
use l402_mock::{EndpointConfig, MockL402Server};

/// Build a server with the given endpoints and return (client, server).
async fn setup(
    endpoints: Vec<(&str, EndpointConfig)>,
    budget: Budget,
    cache_capacity: usize,
) -> (L402Client, MockL402Server) {
    let mut builder = MockL402Server::builder();
    for (path, config) in endpoints {
        builder = builder.endpoint(path, config);
    }
    let server = builder.build().await.expect("mock server should start");
    let backend = server.mock_backend();
    let store = InMemoryTokenStore::new(cache_capacity);

    let client = L402Client::builder()
        .ln_backend(backend)
        .token_store(store)
        .budget(budget)
        .config(L402ClientConfig {
            max_fee_sats: 1000,
            max_retries: 1,
            user_agent: "L402sdk-test/0.1".to_string(),
        })
        .build()
        .expect("client should build");

    (client, server)
}

/// Default setup: one `/api/data` endpoint at 100 sats, unlimited budget, 1024 cache.
async fn default_setup() -> (L402Client, MockL402Server) {
    setup(
        vec![("/api/data", EndpointConfig::new(100))],
        Budget::unlimited(),
        1024,
    )
    .await
}

// ── Happy-path tests ──────────────────────────────────────────────────

#[tokio::test]
async fn happy_path_get() {
    let (client, server) = default_setup().await;
    let url = format!("{}/api/data", server.url());

    let response = client.get(&url).await.expect("request should succeed");

    assert_eq!(response.status().as_u16(), 200);
    assert!(response.paid(), "first request should trigger payment");
    assert!(response.receipt().is_some(), "should have a receipt");

    let receipt = response.receipt().unwrap();
    assert_eq!(receipt.amount_sats, 100);
    assert_eq!(receipt.fee_sats, 0);
    assert_eq!(receipt.response_status, 200);
}

#[tokio::test]
async fn happy_path_post() {
    let (client, server) = setup(
        vec![("/api/submit", EndpointConfig::new(50))],
        Budget::unlimited(),
        1024,
    )
    .await;

    let url = format!("{}/api/submit", server.url());
    let body = r#"{"query":"test"}"#;

    let response = client
        .post(&url, Some(body))
        .await
        .expect("POST should succeed");

    assert_eq!(response.status().as_u16(), 200);
    assert!(response.paid());
}

#[tokio::test]
async fn response_body_returned() {
    let (client, server) = default_setup().await;
    let url = format!("{}/api/data", server.url());

    let response = client.get(&url).await.unwrap();
    let body = response.text().await.unwrap();

    assert!(body.contains(r#""ok":true"#), "body should contain ok:true");
    assert!(
        body.contains(r#""price":100"#),
        "body should contain price:100"
    );
}

#[tokio::test]
async fn custom_response_body() {
    let config = EndpointConfig::new(10).with_body(r#"{"custom":"payload"}"#);
    let (client, server) = setup(vec![("/api/custom", config)], Budget::unlimited(), 1024).await;

    let url = format!("{}/api/custom", server.url());
    let body = client.get(&url).await.unwrap().text().await.unwrap();

    assert_eq!(body, r#"{"custom":"payload"}"#);
}

// ── Token caching tests ───────────────────────────────────────────────

#[tokio::test]
async fn cached_token_avoids_repayment() {
    let (client, server) = default_setup().await;
    let url = format!("{}/api/data", server.url());

    // First request: pays
    let r1 = client.get(&url).await.unwrap();
    assert!(r1.paid());

    // Second request: uses cached token
    let r2 = client.get(&url).await.unwrap();
    assert!(!r2.paid(), "second request should use cached token");
    assert_eq!(r2.status().as_u16(), 200);
}

#[tokio::test]
async fn separate_endpoints_get_separate_tokens() {
    let (client, server) = setup(
        vec![
            ("/api/alpha", EndpointConfig::new(10)),
            ("/api/beta", EndpointConfig::new(20)),
        ],
        Budget::unlimited(),
        1024,
    )
    .await;

    let url_a = format!("{}/api/alpha", server.url());
    let url_b = format!("{}/api/beta", server.url());

    let r1 = client.get(&url_a).await.unwrap();
    assert!(r1.paid());

    let r2 = client.get(&url_b).await.unwrap();
    assert!(
        r2.paid(),
        "different endpoint should require its own payment"
    );

    // Both should now be cached
    let r3 = client.get(&url_a).await.unwrap();
    assert!(!r3.paid());

    let r4 = client.get(&url_b).await.unwrap();
    assert!(!r4.paid());
}

#[tokio::test]
async fn cache_eviction_triggers_repayment() {
    // Cache capacity of 1: second endpoint evicts first
    let (client, server) = setup(
        vec![
            ("/api/one", EndpointConfig::new(10)),
            ("/api/two", EndpointConfig::new(10)),
        ],
        Budget::unlimited(),
        1, // capacity = 1
    )
    .await;

    let url_one = format!("{}/api/one", server.url());
    let url_two = format!("{}/api/two", server.url());

    let r1 = client.get(&url_one).await.unwrap();
    assert!(r1.paid());

    // Accessing /api/two evicts /api/one from cache
    let r2 = client.get(&url_two).await.unwrap();
    assert!(r2.paid());

    // Accessing /api/one again should re-pay since it was evicted
    let r3 = client.get(&url_one).await.unwrap();
    assert!(r3.paid(), "evicted token should trigger re-payment");
}

// ── Budget enforcement tests ──────────────────────────────────────────

#[tokio::test]
async fn budget_per_request_blocks_expensive_request() {
    let budget = Budget {
        per_request_max: Some(50),
        hourly_max: None,
        daily_max: None,
        total_max: None,
        domain_budgets: std::collections::HashMap::new(),
    };

    // Endpoint charges 100 sats, per-request limit is 50 sats.
    // The BOLT11 decoder extracts the real amount, so the budget check blocks it.
    let (client, server) = setup(vec![("/api/data", EndpointConfig::new(100))], budget, 1024).await;

    let url = format!("{}/api/data", server.url());
    let result = client.get(&url).await;

    let Err(err) = result else {
        panic!("expected budget exceeded error, got Ok");
    };
    assert!(
        format!("{err}").contains("budget exceeded"),
        "expected budget exceeded error, got: {err}"
    );
}

#[tokio::test]
async fn budget_per_request_allows_cheap_request() {
    let budget = Budget {
        per_request_max: Some(200),
        hourly_max: None,
        daily_max: None,
        total_max: None,
        domain_budgets: std::collections::HashMap::new(),
    };

    // Endpoint charges 100 sats, per-request limit is 200 sats — should pass.
    let (client, server) = setup(vec![("/api/data", EndpointConfig::new(100))], budget, 1024).await;

    let url = format!("{}/api/data", server.url());
    let result = client.get(&url).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn budget_total_limit_blocks_excess_spending() {
    let budget = Budget {
        per_request_max: None,
        hourly_max: None,
        daily_max: None,
        total_max: Some(0), // Zero total budget
        domain_budgets: std::collections::HashMap::new(),
    };

    let (client, server) = setup(vec![("/api/data", EndpointConfig::new(100))], budget, 1024).await;

    let url = format!("{}/api/data", server.url());
    // With BOLT11 decoding, the 100 sat invoice is correctly decoded and
    // blocked by the zero total budget.
    let result = client.get(&url).await;
    let Err(err) = result else {
        panic!("expected budget exceeded error, got Ok");
    };
    assert!(
        format!("{err}").contains("budget exceeded"),
        "expected budget exceeded error, got: {err}"
    );
}

#[tokio::test]
async fn budget_total_limit_allows_within_budget() {
    let budget = Budget {
        per_request_max: None,
        hourly_max: None,
        daily_max: None,
        total_max: Some(500),
        domain_budgets: std::collections::HashMap::new(),
    };

    let (client, server) = setup(vec![("/api/data", EndpointConfig::new(100))], budget, 1024).await;

    let url = format!("{}/api/data", server.url());
    // 100 sats is within 500 sat total budget
    let result = client.get(&url).await;
    assert!(result.is_ok());
}

// ── Backend failure tests ─────────────────────────────────────────────

#[tokio::test]
async fn insufficient_balance_fails() {
    let (_, server) = default_setup().await;

    // Create a backend with low balance
    let backend = server.mock_backend();
    backend.set_balance(10).await; // Only 10 sats, need 100

    let store = InMemoryTokenStore::new(1024);
    let client = L402Client::builder()
        .ln_backend(backend)
        .token_store(store)
        .budget(Budget::unlimited())
        .build()
        .unwrap();

    let url = format!("{}/api/data", server.url());
    let result = client.get(&url).await;

    let Err(err) = result else {
        panic!("should fail with insufficient balance")
    };
    assert!(
        format!("{err}").contains("insufficient balance"),
        "error should mention insufficient balance, got: {err}"
    );
}

// ── Non-402 passthrough tests ─────────────────────────────────────────

#[tokio::test]
async fn unprotected_404_passthrough() {
    let (client, server) = default_setup().await;
    let url = format!("{}/nonexistent", server.url());

    let response = client.get(&url).await.expect("should not error on 404");

    assert_eq!(response.status().as_u16(), 404);
    assert!(!response.paid(), "404 should not trigger payment");
    assert!(response.receipt().is_none());
}

// ── Receipt tracking tests ────────────────────────────────────────────

#[tokio::test]
async fn receipts_recorded_correctly() {
    let (client, server) = setup(
        vec![
            ("/api/one", EndpointConfig::new(50)),
            ("/api/two", EndpointConfig::new(75)),
        ],
        Budget::unlimited(),
        1024,
    )
    .await;

    // Make two paid requests
    client
        .get(&format!("{}/api/one", server.url()))
        .await
        .unwrap();
    client
        .get(&format!("{}/api/two", server.url()))
        .await
        .unwrap();

    // Third request should be cached (no new receipt)
    client
        .get(&format!("{}/api/one", server.url()))
        .await
        .unwrap();

    let receipts = client.receipts().await;
    assert_eq!(receipts.len(), 2, "should have exactly 2 receipts");

    assert_eq!(receipts[0].amount_sats, 50);
    assert_eq!(receipts[0].response_status, 200);
    assert!(receipts[0].endpoint.ends_with("/api/one"));

    assert_eq!(receipts[1].amount_sats, 75);
    assert_eq!(receipts[1].response_status, 200);
    assert!(receipts[1].endpoint.ends_with("/api/two"));
}

#[tokio::test]
async fn total_spent_via_receipts() {
    let (client, server) = setup(
        vec![
            ("/api/one", EndpointConfig::new(100)),
            ("/api/two", EndpointConfig::new(200)),
        ],
        Budget::unlimited(),
        1024,
    )
    .await;

    client
        .get(&format!("{}/api/one", server.url()))
        .await
        .unwrap();
    client
        .get(&format!("{}/api/two", server.url()))
        .await
        .unwrap();

    let receipts = client.receipts().await;
    let total: u64 = receipts
        .iter()
        .map(l402_core::receipt::Receipt::total_cost_sats)
        .sum();
    assert_eq!(total, 300, "receipts should sum to 300 sats");
}

// ── Concurrent access test ────────────────────────────────────────────

#[tokio::test]
async fn concurrent_requests_to_different_endpoints() {
    let (client, server) = setup(
        vec![
            ("/api/a", EndpointConfig::new(10)),
            ("/api/b", EndpointConfig::new(20)),
            ("/api/c", EndpointConfig::new(30)),
        ],
        Budget::unlimited(),
        1024,
    )
    .await;

    let client = std::sync::Arc::new(client);
    let base_url = server.url();

    let mut handles = Vec::new();
    for path in &["/api/a", "/api/b", "/api/c"] {
        let c = client.clone();
        let url = format!("{base_url}{path}");
        handles.push(tokio::spawn(async move {
            let r = c.get(&url).await.unwrap();
            assert_eq!(r.status().as_u16(), 200);
            assert!(r.paid());
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let receipts = client.receipts().await;
    assert_eq!(receipts.len(), 3);
}
