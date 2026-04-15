//! WASM browser integration tests (run with `wasm-pack test --headless --chrome`).
//!
//! Verifies that the WASM module loads correctly in a browser environment
//! and that core types can be constructed.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

use l402_wasm::*;

// ---------------------------------------------------------------------------
// Panic hook
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn panic_hook_does_not_throw() {
    set_panic_hook();
}

// ---------------------------------------------------------------------------
// Budget configuration
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn budget_config_unlimited() {
    let budget = WasmBudgetConfig::unlimited();
    assert_eq!(budget.per_request_max, 0);
    assert_eq!(budget.hourly_max, 0);
    assert_eq!(budget.daily_max, 0);
    assert_eq!(budget.total_max, 0);
}

#[wasm_bindgen_test]
fn budget_config_with_limits() {
    let budget = WasmBudgetConfig::new(1000, 5000, 50000, 1000000);
    assert_eq!(budget.per_request_max, 1000);
    assert_eq!(budget.hourly_max, 5000);
    assert_eq!(budget.daily_max, 50000);
    assert_eq!(budget.total_max, 1000000);
}

// ---------------------------------------------------------------------------
// Backend construction
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn lnd_rest_backend_constructs() {
    let backend = WasmLndRestBackend::new("https://localhost:8080", "deadbeefcafebabe");
    assert!(backend.is_ok());
}

#[wasm_bindgen_test]
fn swissknife_backend_constructs() {
    let _backend = WasmSwissKnifeBackend::new("https://api.numeraire.tech", "sk-test");
}

#[wasm_bindgen_test]
fn cln_rest_backend_constructs() {
    let backend = WasmClnRestBackend::new("https://localhost:3001", "deadbeefcafebabe");
    assert!(backend.is_ok());
}

// ---------------------------------------------------------------------------
// L402 client construction
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn l402_client_with_lnd_rest() {
    let budget = WasmBudgetConfig::unlimited();
    let client =
        WasmL402Client::with_lnd_rest("https://localhost:8080", "deadbeefcafebabe", budget, 100);
    assert!(client.is_ok());
}

#[wasm_bindgen_test]
fn l402_client_with_swissknife() {
    let budget = WasmBudgetConfig::unlimited();
    let client =
        WasmL402Client::with_swissknife("https://api.numeraire.tech", "sk-test", budget, 100);
    assert!(client.is_ok());
}
