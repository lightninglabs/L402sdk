//! Full L402 protocol flow using the LND gRPC backend.
//!
//! Tests the complete path: GET → 402 → pay invoice → retry with token → 200.

use regtest_helpers::*;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[tokio::test]
async fn full_l402_flow_grpc() {
    skip_if_no_regtest!();

    let backend = lnd_grpc_backend().await;

    // Verify backend connectivity first
    let info = l402_proto::LnBackend::get_info(&backend).await.unwrap();
    tracing::info!("Connected to LND: {} ({})", info.alias, info.pubkey);
    assert!(!info.pubkey.is_empty());
    assert!(
        info.num_active_channels > 0,
        "LND must have active channels"
    );

    // Check balance
    let balance = l402_proto::LnBackend::get_balance(&backend).await.unwrap();
    tracing::info!("LND balance: {} sats", balance);
    assert!(balance > 1000, "LND must have funds to pay invoices");

    // Build L402 client with the gRPC backend
    let client = build_l402_client(backend);

    // Full L402 flow against the test server
    let url = format!("{}/api/data", l402_server_url());
    let response = client.get(&url).await.unwrap();

    // Should have paid and gotten 200
    assert_eq!(response.status().as_u16(), 200);
    assert!(response.paid(), "request should have triggered a payment");
    assert!(
        !response.cached_token(),
        "first request should not use cached token"
    );

    // Verify receipt
    let receipt = response
        .receipt()
        .expect("paid request must have a receipt");
    assert_eq!(receipt.amount_sats, 100, "endpoint price is 100 sats");
    assert_eq!(receipt.response_status, 200);
    assert!(!receipt.payment_hash.is_empty());
    assert!(!receipt.preimage.is_empty());
    assert!(receipt.latency_ms > 0);

    // Read the response body
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["ok"], true);
    assert_eq!(body["resource"], "data");
}

#[tokio::test]
async fn grpc_premium_endpoint() {
    skip_if_no_regtest!();

    let backend = lnd_grpc_backend().await;
    let client = build_l402_client(backend);

    let url = format!("{}/api/premium", l402_server_url());
    let response = client.get(&url).await.unwrap();

    assert_eq!(response.status().as_u16(), 200);
    assert!(response.paid());

    let receipt = response.receipt().unwrap();
    assert_eq!(receipt.amount_sats, 500, "premium endpoint is 500 sats");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["resource"], "premium");
}

#[tokio::test]
async fn grpc_nonexistent_endpoint() {
    skip_if_no_regtest!();

    let backend = lnd_grpc_backend().await;
    let client = build_l402_client(backend);

    let url = format!("{}/api/nonexistent", l402_server_url());
    let response = client.get(&url).await.unwrap();

    // The L402 server returns 404, client should pass it through
    assert_eq!(response.status().as_u16(), 404);
    assert!(!response.paid(), "no payment for 404 responses");
}

#[tokio::test]
async fn grpc_receipt_preimage_matches_hash() {
    skip_if_no_regtest!();

    let backend = lnd_grpc_backend().await;
    let client = build_l402_client(backend);

    let url = format!("{}/api/cheap", l402_server_url());
    let response = client.get(&url).await.unwrap();

    assert_eq!(response.status().as_u16(), 200);
    let receipt = response.receipt().unwrap();

    // Verify SHA256(preimage) == payment_hash
    let preimage_bytes = hex::decode(&receipt.preimage).unwrap();
    let computed_hash = hex::encode(Sha256::digest(&preimage_bytes));
    assert_eq!(
        computed_hash, receipt.payment_hash,
        "SHA256(preimage) must equal payment_hash"
    );

    assert_eq!(receipt.amount_sats, 10, "cheap endpoint is 10 sats");
}
