//! Full L402 protocol flow using the LND REST backend.
//!
//! Mirrors lnd_grpc_flow.rs but uses the REST adapter, validating both
//! backend implementations against the same L402 test server.

use regtest_helpers::*;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[tokio::test]
async fn full_l402_flow_rest() {
    skip_if_no_regtest!();

    let backend = lnd_rest_backend();

    // Verify REST backend connectivity
    let info = l402_proto::LnBackend::get_info(&backend).await.unwrap();
    tracing::info!("Connected to LND (REST): {} ({})", info.alias, info.pubkey);
    assert!(!info.pubkey.is_empty());
    assert!(
        info.num_active_channels > 0,
        "LND must have active channels"
    );

    let balance = l402_proto::LnBackend::get_balance(&backend).await.unwrap();
    tracing::info!("LND balance (REST): {} sats", balance);
    assert!(balance > 1000);

    let client = build_l402_client(backend);

    let url = format!("{}/api/data", l402_server_url());
    let response = client.get(&url).await.unwrap();

    assert_eq!(response.status().as_u16(), 200);
    assert!(response.paid());
    assert!(!response.cached_token());

    let receipt = response.receipt().unwrap();
    assert_eq!(receipt.amount_sats, 100);
    assert_eq!(receipt.response_status, 200);
    assert!(!receipt.payment_hash.is_empty());
    assert!(!receipt.preimage.is_empty());

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["ok"], true);
}

#[tokio::test]
async fn rest_premium_endpoint() {
    skip_if_no_regtest!();

    let backend = lnd_rest_backend();
    let client = build_l402_client(backend);

    let url = format!("{}/api/premium", l402_server_url());
    let response = client.get(&url).await.unwrap();

    assert_eq!(response.status().as_u16(), 200);
    assert!(response.paid());

    let receipt = response.receipt().unwrap();
    assert_eq!(receipt.amount_sats, 500);
}

#[tokio::test]
async fn rest_nonexistent_endpoint() {
    skip_if_no_regtest!();

    let backend = lnd_rest_backend();
    let client = build_l402_client(backend);

    let url = format!("{}/api/nonexistent", l402_server_url());
    let response = client.get(&url).await.unwrap();

    assert_eq!(response.status().as_u16(), 404);
    assert!(!response.paid());
}

#[tokio::test]
async fn rest_receipt_preimage_matches_hash() {
    skip_if_no_regtest!();

    let backend = lnd_rest_backend();
    let client = build_l402_client(backend);

    let url = format!("{}/api/cheap", l402_server_url());
    let response = client.get(&url).await.unwrap();

    assert_eq!(response.status().as_u16(), 200);
    let receipt = response.receipt().unwrap();

    let preimage_bytes = hex::decode(&receipt.preimage).unwrap();
    let computed_hash = hex::encode(Sha256::digest(&preimage_bytes));
    assert_eq!(computed_hash, receipt.payment_hash);

    assert_eq!(receipt.amount_sats, 10);
}

#[tokio::test]
async fn rest_multiple_sequential_payments() {
    skip_if_no_regtest!();

    let backend = lnd_rest_backend();
    let client = build_l402_client(backend);

    // Pay for three different endpoints in sequence
    let endpoints = [
        ("/api/cheap", 10),
        ("/api/data", 100),
        ("/api/premium", 500),
    ];

    for (path, expected_sats) in &endpoints {
        let url = format!("{}{}", l402_server_url(), path);
        let response = client.get(&url).await.unwrap();
        assert_eq!(response.status().as_u16(), 200);
        assert!(response.paid());

        let receipt = response.receipt().unwrap();
        assert_eq!(receipt.amount_sats, *expected_sats);
    }

    // Verify total spending matches
    let total = client.total_spent().await;
    assert_eq!(total, 610, "total should be 10 + 100 + 500 = 610 sats");
}
