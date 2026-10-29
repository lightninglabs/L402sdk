//! Budget enforcement tests with real Lightning payments.
//!
//! Validates that the budget system correctly prevents payments that exceed
//! configured limits, using real invoices and the regtest Lightning network.

use l402_core::budget::Budget;
use regtest_helpers::*;

#[tokio::test]
async fn per_request_budget_allows_cheap() {
    skip_if_no_regtest!();

    let backend = lnd_rest_backend();
    let budget = Budget {
        per_request_max: Some(50),
        ..Budget::unlimited()
    };

    let client = build_l402_client_with_budget(backend, budget);

    // 10-sat endpoint should be within budget
    let url = format!("{}/api/cheap", l402_server_url());
    let response = client.get(&url).await.unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert!(response.paid());
}

#[tokio::test]
async fn per_request_budget_blocks_expensive() {
    skip_if_no_regtest!();

    let backend = lnd_rest_backend();
    let budget = Budget {
        per_request_max: Some(50),
        ..Budget::unlimited()
    };

    let client = build_l402_client_with_budget(backend, budget);

    // 100-sat endpoint should exceed per-request limit
    let url = format!("{}/api/data", l402_server_url());
    let result = client.get(&url).await;
    assert!(
        result.is_err(),
        "should reject payment exceeding per-request budget"
    );

    let err = result.err().expect("expected budget error");
    let err_str = format!("{err}");
    assert!(
        err_str.contains("budget") || err_str.contains("Budget"),
        "error should mention budget: {err_str}"
    );
}

#[tokio::test]
async fn total_budget_enforced_across_requests() {
    skip_if_no_regtest!();

    let backend = lnd_rest_backend();
    let budget = Budget {
        total_max: Some(150),
        ..Budget::unlimited()
    };

    let client = build_l402_client_with_budget(backend, budget);

    // First request: 100 sats (total: 100, within budget)
    let url = format!("{}/api/data", l402_server_url());
    let response = client.get(&url).await.unwrap();
    assert_eq!(response.status().as_u16(), 200);

    // Second request: 100 sats would bring total to 200, exceeding 150 limit
    // Note: the token for /api/data is cached, so we need a different endpoint
    let url2 = format!("{}/api/premium", l402_server_url());
    let result = client.get(&url2).await;
    assert!(result.is_err(), "second payment should exceed total budget");
}

#[tokio::test]
async fn unlimited_budget_allows_everything() {
    skip_if_no_regtest!();

    let backend = lnd_rest_backend();
    let client = build_l402_client_with_budget(backend, Budget::unlimited());

    // All endpoints should work
    for path in &["/api/cheap", "/api/data", "/api/premium"] {
        let url = format!("{}{}", l402_server_url(), path);
        let response = client.get(&url).await.unwrap();
        assert_eq!(response.status().as_u16(), 200);
    }
}

async fn assert_zero_fee_payment<B: l402_proto::LnBackend + 'static>(backend: B) {
    let client = l402_core::L402Client::builder()
        .ln_backend(backend)
        .token_store(l402_core::cache::InMemoryTokenStore::default())
        .config(l402_core::L402ClientConfig {
            max_fee_sats: 0,
            ..Default::default()
        })
        .budget(Budget {
            total_max: Some(10),
            ..Budget::unlimited()
        })
        .build()
        .unwrap();
    let response = client
        .get(&format!("{}/api/cheap", l402_server_url()))
        .await
        .unwrap();
    let receipt = response.receipt().unwrap();
    assert_eq!(receipt.amount_sats, 10);
    assert_eq!(receipt.fee_sats, 0);
    assert_eq!(client.total_spent().await, 10);
}

#[tokio::test]
async fn explicit_zero_fee_cap_works_on_lnd_and_cln_rest_and_grpc() {
    skip_if_no_regtest!();
    assert_zero_fee_payment(lnd_rest_backend()).await;
    assert_zero_fee_payment(lnd_grpc_backend().await).await;
    assert_zero_fee_payment(cln_rest_backend().await.expect("CLN REST required")).await;
    assert_zero_fee_payment(cln_backend().await.expect("CLN gRPC required")).await;
}

#[tokio::test]
async fn concurrent_funded_payments_cannot_overdraw_budget() {
    skip_if_no_regtest!();
    let client = build_l402_client_with_budget(
        lnd_rest_backend(),
        Budget {
            total_max: Some(150),
            ..Budget::unlimited()
        },
    );
    let url = format!("{}/api/data", l402_server_url());
    let (first, second) = tokio::join!(client.get(&url), client.get(&url));
    // If a request observes the newly cached proof, it can succeed without paying.
    assert!(first.is_ok() || second.is_ok());
    let receipts = client.receipts().await;
    assert_eq!(receipts.len(), 1);
    assert_eq!(client.total_spent().await, receipts[0].total_cost_sats());
    assert!(client.total_spent().await <= 150);
}
