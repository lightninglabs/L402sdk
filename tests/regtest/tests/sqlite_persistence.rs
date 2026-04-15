//! SQLite token store persistence tests with real Lightning payments.
//!
//! Validates that tokens persisted in SQLite survive client restarts
//! and correctly skip re-payment when the token is still valid.

use l402_sqlite::SqliteTokenStore;
use regtest_helpers::*;
use tempfile::NamedTempFile;

#[tokio::test]
async fn sqlite_token_survives_restart() {
    skip_if_no_regtest!();

    let db_file = NamedTempFile::new().unwrap();
    let db_path = db_file.path().to_str().unwrap().to_string();

    let url = format!("{}/api/data", l402_server_url());

    // First client: pays and stores token in SQLite
    {
        let store = SqliteTokenStore::new(&db_path).unwrap();
        let backend = lnd_rest_backend();
        let client = build_l402_client_with_store(backend, store);

        let resp = client.get(&url).await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        assert!(resp.paid(), "first client should pay");
        let _ = resp.text().await.unwrap();
    }
    // First client dropped — simulates process restart

    // Second client: opens same SQLite DB, should reuse cached token
    {
        let store = SqliteTokenStore::new(&db_path).unwrap();
        let backend = lnd_rest_backend();
        let client = build_l402_client_with_store(backend, store);

        let resp = client.get(&url).await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        assert!(
            !resp.paid(),
            "second client should use persisted token, not pay again"
        );
        assert!(resp.cached_token());
    }
}

#[tokio::test]
async fn sqlite_separate_dbs_require_separate_payments() {
    skip_if_no_regtest!();

    let db1 = NamedTempFile::new().unwrap();
    let db2 = NamedTempFile::new().unwrap();

    let url = format!("{}/api/cheap", l402_server_url());

    // Client 1 with db1
    let store1 = SqliteTokenStore::new(db1.path().to_str().unwrap()).unwrap();
    let client1 = build_l402_client_with_store(lnd_rest_backend(), store1);
    let resp1 = client1.get(&url).await.unwrap();
    assert!(resp1.paid());
    let _ = resp1.text().await.unwrap();

    // Client 2 with db2 (different database)
    let store2 = SqliteTokenStore::new(db2.path().to_str().unwrap()).unwrap();
    let client2 = build_l402_client_with_store(lnd_rest_backend(), store2);
    let resp2 = client2.get(&url).await.unwrap();
    assert!(resp2.paid(), "separate DB should not have cached token");
    let _ = resp2.text().await.unwrap();
}

#[tokio::test]
async fn sqlite_multiple_endpoints_persisted() {
    skip_if_no_regtest!();

    let db_file = NamedTempFile::new().unwrap();
    let db_path = db_file.path().to_str().unwrap().to_string();

    // Pay for multiple endpoints
    {
        let store = SqliteTokenStore::new(&db_path).unwrap();
        let backend = lnd_rest_backend();
        let client = build_l402_client_with_store(backend, store);

        for path in &["/api/cheap", "/api/data"] {
            let url = format!("{}{}", l402_server_url(), path);
            let resp = client.get(&url).await.unwrap();
            assert!(resp.paid());
            let _ = resp.text().await.unwrap();
        }
    }

    // Restart: both tokens should be cached
    {
        let store = SqliteTokenStore::new(&db_path).unwrap();
        let backend = lnd_rest_backend();
        let client = build_l402_client_with_store(backend, store);

        for path in &["/api/cheap", "/api/data"] {
            let url = format!("{}{}", l402_server_url(), path);
            let resp = client.get(&url).await.unwrap();
            assert!(
                resp.cached_token(),
                "endpoint {} should use cached token after restart",
                path
            );
            assert!(!resp.paid());
            let _ = resp.text().await.unwrap();
        }
    }
}
