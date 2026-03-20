//! WASM integration tests (run with `wasm-pack test --headless --chrome`).

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

use bolt402_wasm::*;

// ---------------------------------------------------------------------------
// Helper: create a server from a JsValue map
// ---------------------------------------------------------------------------

fn make_server(endpoints: &[(&str, u64)]) -> WasmMockServer {
    let obj = js_sys::Object::new();
    for (path, price) in endpoints {
        js_sys::Reflect::set(
            &obj,
            &JsValue::from_str(path),
            &JsValue::from_f64(*price as f64),
        )
        .unwrap();
    }
    WasmMockServer::new(obj.into()).unwrap()
}

// ---------------------------------------------------------------------------
// Server tests
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn server_creation() {
    let server = make_server(&[("/api/data", 10), ("/api/premium", 100)]);
    let paths = server.endpoint_paths();
    assert_eq!(paths.len(), 2);
    assert_eq!(server.balance(), 1_000_000);
}

#[wasm_bindgen_test]
fn server_balance_management() {
    let server = make_server(&[("/api/data", 10)]);
    assert_eq!(server.balance(), 1_000_000);

    server.set_balance(500);
    assert_eq!(server.balance(), 500);
}

// ---------------------------------------------------------------------------
// Client tests
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn client_get_with_payment() {
    let server = make_server(&[("/api/data", 10)]);
    let client = WasmMockClient::new(server, 100);

    let resp = client.get("/api/data").unwrap();
    assert_eq!(resp.status, 200);
    assert!(resp.paid);

    let receipt = resp.receipt().unwrap();
    assert_eq!(receipt.amount_sats, 10);
    assert_eq!(receipt.fee_sats, 0);
    assert_eq!(receipt.total_cost_sats(), 10);
    assert_eq!(receipt.response_status, 200);
    assert!(!receipt.payment_hash().is_empty());
    assert!(!receipt.preimage().is_empty());
}

#[wasm_bindgen_test]
fn client_post_with_payment() {
    let server = make_server(&[("/api/data", 10)]);
    let client = WasmMockClient::new(server, 100);

    let resp = client.post("/api/data").unwrap();
    assert_eq!(resp.status, 200);
    assert!(resp.paid);
}

#[wasm_bindgen_test]
fn client_404_no_payment() {
    let server = make_server(&[("/api/data", 10)]);
    let client = WasmMockClient::new(server, 100);

    let resp = client.get("/unknown").unwrap();
    assert_eq!(resp.status, 404);
    assert!(!resp.paid);
    assert!(resp.receipt().is_none());
}

#[wasm_bindgen_test]
fn client_token_caching() {
    let server = make_server(&[("/api/data", 10)]);
    let client = WasmMockClient::new(server, 100);

    // First request: pays
    let resp1 = client.get("/api/data").unwrap();
    assert!(resp1.paid);
    assert_eq!(client.total_spent(), 10);

    // Second request: uses cached token (no payment)
    let resp2 = client.get("/api/data").unwrap();
    assert!(!resp2.paid);
    assert_eq!(resp2.status, 200);
    assert_eq!(client.total_spent(), 10); // unchanged

    assert_eq!(client.payment_count(), 1);
}

#[wasm_bindgen_test]
fn client_clear_cache() {
    let server = make_server(&[("/api/data", 10)]);
    let client = WasmMockClient::new(server, 100);

    client.get("/api/data").unwrap();
    assert_eq!(client.payment_count(), 1);

    client.clear_cache();

    // After clearing, must pay again
    client.get("/api/data").unwrap();
    assert_eq!(client.payment_count(), 2);
    assert_eq!(client.total_spent(), 20);
}

#[wasm_bindgen_test]
fn client_receipts() {
    let server = make_server(&[("/api/a", 10), ("/api/b", 20)]);
    let client = WasmMockClient::new(server, 100);

    client.get("/api/a").unwrap();
    client.get("/api/b").unwrap();

    let receipts = client.receipts();
    assert_eq!(receipts.len(), 2);
    assert_eq!(receipts[0].amount_sats, 10);
    assert_eq!(receipts[0].endpoint(), "/api/a");
    assert_eq!(receipts[1].amount_sats, 20);
    assert_eq!(receipts[1].endpoint(), "/api/b");

    assert_eq!(client.total_spent(), 30);
}

#[wasm_bindgen_test]
fn client_server_balance_decreases() {
    let server = make_server(&[("/api/data", 100)]);
    let client = WasmMockClient::new(server, 100);

    assert_eq!(client.server_balance(), 1_000_000);
    client.get("/api/data").unwrap();
    assert_eq!(client.server_balance(), 999_900);
}

// ---------------------------------------------------------------------------
// Budget tests
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn budget_per_request_enforcement() {
    let server = make_server(&[("/api/cheap", 10), ("/api/expensive", 200)]);
    let budget = WasmBudget::new(Some(100), None, None, None);
    let client = WasmMockClient::with_budget(server, 100, budget);

    // Under limit: should work
    let resp = client.get("/api/cheap").unwrap();
    assert_eq!(resp.status, 200);

    // Over per-request limit: should fail
    let result = client.get("/api/expensive");
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn budget_total_enforcement() {
    let server = make_server(&[("/api/data", 30)]);
    let budget = WasmBudget::new(None, None, None, Some(50));
    let client = WasmMockClient::with_budget(server, 100, budget);

    // First: 30 sats, total = 30
    let resp = client.get("/api/data").unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(client.total_spent(), 30);

    // Clear cache to force re-payment
    client.clear_cache();

    // Second: 30 sats, total would be 60 > 50 limit
    let result = client.get("/api/data");
    assert!(result.is_err());
    assert_eq!(client.total_spent(), 30); // unchanged
}

#[wasm_bindgen_test]
fn budget_unlimited_allows_everything() {
    let server = make_server(&[("/api/data", 10)]);
    let budget = WasmBudget::unlimited();
    let client = WasmMockClient::with_budget(server, 100, budget);

    for _ in 0..10 {
        client.clear_cache();
        let resp = client.get("/api/data").unwrap();
        assert_eq!(resp.status, 200);
    }

    assert_eq!(client.total_spent(), 100);
}

// ---------------------------------------------------------------------------
// Insufficient balance tests
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn insufficient_balance_fails() {
    let server = make_server(&[("/api/data", 100)]);
    server.set_balance(50); // Only 50 sats
    let client = WasmMockClient::new(server, 100);

    let result = client.get("/api/data");
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Utility function tests
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn parse_l402_challenge_valid() {
    let header = r#"L402 macaroon="YWJjZGVm", invoice="lnbc100n1pj9nr7mpp5test""#;
    let result = parse_l402_challenge(header).unwrap();

    let macaroon = js_sys::Reflect::get(&result, &JsValue::from_str("macaroon")).unwrap();
    let invoice = js_sys::Reflect::get(&result, &JsValue::from_str("invoice")).unwrap();

    assert_eq!(macaroon.as_string().unwrap(), "YWJjZGVm");
    assert_eq!(invoice.as_string().unwrap(), "lnbc100n1pj9nr7mpp5test");
}

#[wasm_bindgen_test]
fn parse_l402_challenge_invalid() {
    let result = parse_l402_challenge("invalid header");
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn build_l402_header_test() {
    let header = build_l402_header("YWJjZGVm", "abcdef1234567890");
    assert_eq!(header, "L402 YWJjZGVm:abcdef1234567890");
}

#[wasm_bindgen_test]
fn version_returns_string() {
    let v = version();
    assert!(!v.is_empty());
}

// ---------------------------------------------------------------------------
// Response body tests
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn response_body_content() {
    let server = make_server(&[("/api/data", 10)]);
    let client = WasmMockClient::new(server, 100);

    let resp = client.get("/api/data").unwrap();
    let body = resp.body();
    assert!(body.contains("\"ok\":true"));
    assert!(body.contains("\"price\":10"));
}

// ---------------------------------------------------------------------------
// Multiple endpoints test
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn multiple_endpoints_independent() {
    let server = make_server(&[("/api/a", 10), ("/api/b", 20), ("/api/c", 50)]);
    let client = WasmMockClient::new(server, 100);

    let resp_a = client.get("/api/a").unwrap();
    assert_eq!(resp_a.status, 200);
    assert!(resp_a.paid);

    let resp_b = client.get("/api/b").unwrap();
    assert_eq!(resp_b.status, 200);
    assert!(resp_b.paid);

    let resp_c = client.get("/api/c").unwrap();
    assert_eq!(resp_c.status, 200);
    assert!(resp_c.paid);

    assert_eq!(client.total_spent(), 80);
    assert_eq!(client.payment_count(), 3);
}
