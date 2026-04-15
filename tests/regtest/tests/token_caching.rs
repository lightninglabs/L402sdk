//! Token caching tests with real Lightning payments.
//!
//! Validates that L402 tokens are correctly cached and reused on subsequent
//! requests, avoiding unnecessary re-payment.

use l402_core::cache::InMemoryTokenStore;
use regtest_helpers::*;

#[tokio::test]
async fn cached_token_skips_payment() {
    skip_if_no_regtest!();

    let backend = lnd_rest_backend();
    let client = build_l402_client(backend);

    let url = format!("{}/api/data", l402_server_url());

    // First request: triggers payment
    let resp1 = client.get(&url).await.unwrap();
    assert_eq!(resp1.status().as_u16(), 200);
    assert!(resp1.paid(), "first request should pay");
    assert!(!resp1.cached_token(), "first request should not use cache");
    let _ = resp1.text().await.unwrap(); // consume body

    // Second request to same endpoint: should use cached token
    let resp2 = client.get(&url).await.unwrap();
    assert_eq!(resp2.status().as_u16(), 200);
    assert!(!resp2.paid(), "second request should NOT pay");
    assert!(
        resp2.cached_token(),
        "second request should use cached token"
    );
    assert!(
        resp2.receipt().is_none(),
        "cached request should have no receipt"
    );
}

#[tokio::test]
async fn different_endpoints_not_cached() {
    skip_if_no_regtest!();

    let backend = lnd_rest_backend();
    let client = build_l402_client(backend);

    // Request /api/cheap — pays
    let url1 = format!("{}/api/cheap", l402_server_url());
    let resp1 = client.get(&url1).await.unwrap();
    assert!(resp1.paid());
    let _ = resp1.text().await.unwrap();

    // Request /api/data — different endpoint, must pay again
    let url2 = format!("{}/api/data", l402_server_url());
    let resp2 = client.get(&url2).await.unwrap();
    assert!(
        resp2.paid(),
        "different endpoint should require separate payment"
    );
    assert!(!resp2.cached_token());
}

#[tokio::test]
async fn total_spent_tracks_real_payments() {
    skip_if_no_regtest!();

    let backend = lnd_rest_backend();
    let client = build_l402_client(backend);

    let url = format!("{}/api/cheap", l402_server_url());

    // First request pays
    let resp1 = client.get(&url).await.unwrap();
    assert!(resp1.paid());
    let _ = resp1.text().await.unwrap();

    let total_after_first = client.total_spent().await;
    assert_eq!(total_after_first, 10, "should have spent 10 sats");

    // Second request uses cache — no additional spending
    let resp2 = client.get(&url).await.unwrap();
    assert!(resp2.cached_token());
    let _ = resp2.text().await.unwrap();

    let total_after_second = client.total_spent().await;
    assert_eq!(
        total_after_second, 10,
        "cached request should not increase total spent"
    );
}

#[tokio::test]
async fn receipts_only_for_paid_requests() {
    skip_if_no_regtest!();

    let backend = lnd_rest_backend();
    let client = build_l402_client(backend);

    let url = format!("{}/api/data", l402_server_url());

    // Pay once
    let resp = client.get(&url).await.unwrap();
    let _ = resp.text().await.unwrap();

    // Use cache
    let resp2 = client.get(&url).await.unwrap();
    let _ = resp2.text().await.unwrap();

    // Should have exactly one receipt (only the paid request)
    let receipts = client.receipts().await;
    assert_eq!(receipts.len(), 1, "should have exactly 1 receipt (not 2)");
    assert_eq!(receipts[0].amount_sats, 100);
}

#[tokio::test]
async fn empty_store_forces_payment() {
    skip_if_no_regtest!();

    // Two separate clients with separate token stores
    let backend1 = lnd_rest_backend();
    let client1 = build_l402_client_with_store(backend1, InMemoryTokenStore::default());

    let backend2 = lnd_rest_backend();
    let client2 = build_l402_client_with_store(backend2, InMemoryTokenStore::default());

    let url = format!("{}/api/cheap", l402_server_url());

    // Both clients must pay independently (separate stores)
    let resp1 = client1.get(&url).await.unwrap();
    assert!(resp1.paid());
    let _ = resp1.text().await.unwrap();

    let resp2 = client2.get(&url).await.unwrap();
    assert!(resp2.paid(), "client2 has empty store, must pay");
    let _ = resp2.text().await.unwrap();
}
