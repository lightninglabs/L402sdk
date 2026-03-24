//! WASM-bindgen wrapper for the Rust [`bolt402_core::L402Client`] from `bolt402-core`.
//!
//! Exposes the full L402 protocol engine to JavaScript/TypeScript via
//! `wasm-bindgen`. The client handles HTTP 402 challenges, Lightning
//! payments, token caching, budget enforcement, and receipt tracking
//! entirely in Rust compiled to WASM.

use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use bolt402_core::budget::Budget;
use bolt402_core::cache::InMemoryTokenStore;
use bolt402_core::{L402Client, L402ClientConfig};
use bolt402_lnd::LndRestBackend;
use bolt402_swissknife::SwissKnifeBackend;

use crate::WasmReceipt;

// ---------------------------------------------------------------------------
// WasmBudgetConfig
// ---------------------------------------------------------------------------

/// Budget configuration for the L402 client.
///
/// All limits are optional. Pass `0` for no limit on that granularity.
/// Amounts are in satoshis.
#[wasm_bindgen]
#[derive(Debug, Clone, Default)]
pub struct WasmBudgetConfig {
    /// Maximum per-request amount in satoshis, or 0 for no limit.
    #[wasm_bindgen(readonly, js_name = "perRequestMax")]
    pub per_request_max: u64,
    /// Maximum hourly amount in satoshis, or 0 for no limit.
    #[wasm_bindgen(readonly, js_name = "hourlyMax")]
    pub hourly_max: u64,
    /// Maximum daily amount in satoshis, or 0 for no limit.
    #[wasm_bindgen(readonly, js_name = "dailyMax")]
    pub daily_max: u64,
    /// Maximum total amount in satoshis, or 0 for no limit.
    #[wasm_bindgen(readonly, js_name = "totalMax")]
    pub total_max: u64,
}

#[wasm_bindgen]
impl WasmBudgetConfig {
    /// Create a new budget configuration.
    ///
    /// Pass `0` for any limit to leave it unlimited.
    #[wasm_bindgen(constructor)]
    pub fn new(per_request_max: u64, hourly_max: u64, daily_max: u64, total_max: u64) -> Self {
        Self {
            per_request_max,
            hourly_max,
            daily_max,
            total_max,
        }
    }

    /// Create an unlimited budget (no restrictions).
    pub fn unlimited() -> Self {
        Self::default()
    }
}

impl From<WasmBudgetConfig> for Budget {
    fn from(config: WasmBudgetConfig) -> Self {
        let to_opt = |v: u64| if v == 0 { None } else { Some(v) };
        Budget {
            per_request_max: to_opt(config.per_request_max),
            hourly_max: to_opt(config.hourly_max),
            daily_max: to_opt(config.daily_max),
            total_max: to_opt(config.total_max),
            domain_budgets: std::collections::HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// WasmL402Response
// ---------------------------------------------------------------------------

/// Response from an L402-aware HTTP request.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct WasmL402Response {
    /// HTTP status code.
    #[wasm_bindgen(readonly)]
    pub status: u16,
    /// Whether a Lightning payment was made.
    #[wasm_bindgen(readonly)]
    pub paid: bool,
    /// Whether a cached L402 token was used (no new payment needed).
    #[wasm_bindgen(readonly, js_name = "cachedToken")]
    pub cached_token: bool,
    body: String,
    receipt: Option<WasmReceipt>,
}

#[wasm_bindgen]
impl WasmL402Response {
    /// The response body as a string.
    #[wasm_bindgen(getter)]
    pub fn body(&self) -> String {
        self.body.clone()
    }

    /// The payment receipt, or `undefined` if no payment was made.
    #[wasm_bindgen(getter)]
    pub fn receipt(&self) -> Option<WasmReceipt> {
        self.receipt.clone()
    }
}

// ---------------------------------------------------------------------------
// WasmL402Client
// ---------------------------------------------------------------------------

/// L402 client that handles the full payment-gated HTTP flow.
///
/// Wraps the Rust `L402Client` from `bolt402-core`. All protocol logic
/// (challenge parsing, budget enforcement, token caching, receipt tracking)
/// runs in Rust/WASM.
///
/// # Example (LND REST)
///
/// ```javascript
/// const client = WasmL402Client.withLndRest(
///   "https://localhost:8080",
///   "deadbeef...",
///   WasmBudgetConfig.unlimited(),
///   100,
/// );
///
/// const response = await client.get("https://api.example.com/data");
/// console.log(response.status, response.paid);
/// ```
#[wasm_bindgen]
pub struct WasmL402Client {
    // Rc because wasm-bindgen does not support lifetimes and we need to
    // share the client across multiple async calls. WASM is single-threaded
    // so Rc is safe. Same pattern as bdk-wasm's Wallet(Rc<RefCell<BdkWallet>>).
    inner: Rc<L402Client>,
}

#[wasm_bindgen]
impl WasmL402Client {
    /// Create an L402 client backed by LND REST.
    ///
    /// # Arguments
    ///
    /// * `url` - LND REST API URL (e.g. `https://localhost:8080`)
    /// * `macaroon` - Hex-encoded admin macaroon
    /// * `budget` - Budget configuration (use `WasmBudgetConfig.unlimited()` for no limits)
    /// * `max_fee_sats` - Maximum routing fee in satoshis
    #[wasm_bindgen(js_name = "withLndRest")]
    pub fn with_lnd_rest(
        url: &str,
        macaroon: &str,
        budget: WasmBudgetConfig,
        max_fee_sats: u64,
    ) -> Result<WasmL402Client, JsError> {
        let backend = LndRestBackend::new(url, macaroon)
            .map_err(|e| JsError::new(&format!("failed to create LND backend: {e}")))?;

        let client = L402Client::builder()
            .ln_backend(backend)
            .token_store(InMemoryTokenStore::default())
            .budget(budget.into())
            .config(L402ClientConfig {
                max_fee_sats,
                ..L402ClientConfig::default()
            })
            .build()
            .map_err(|e| JsError::new(&format!("failed to build L402Client: {e}")))?;

        Ok(Self {
            inner: Rc::new(client),
        })
    }

    /// Create an L402 client backed by SwissKnife REST.
    ///
    /// # Arguments
    ///
    /// * `url` - SwissKnife API URL (e.g. `https://app.numeraire.tech`)
    /// * `api_key` - API key for authentication
    /// * `budget` - Budget configuration
    /// * `max_fee_sats` - Maximum routing fee in satoshis
    #[wasm_bindgen(js_name = "withSwissKnife")]
    pub fn with_swissknife(
        url: &str,
        api_key: &str,
        budget: WasmBudgetConfig,
        max_fee_sats: u64,
    ) -> Result<WasmL402Client, JsError> {
        let backend = SwissKnifeBackend::new(url, api_key);

        let client = L402Client::builder()
            .ln_backend(backend)
            .token_store(InMemoryTokenStore::default())
            .budget(budget.into())
            .config(L402ClientConfig {
                max_fee_sats,
                ..L402ClientConfig::default()
            })
            .build()
            .map_err(|e| JsError::new(&format!("failed to build L402Client: {e}")))?;

        Ok(Self {
            inner: Rc::new(client),
        })
    }

    /// Send a GET request, automatically handling L402 payment challenges.
    ///
    /// Returns a `Promise<WasmL402Response>`.
    pub fn get(&self, url: &str) -> js_sys::Promise {
        let url = url.to_string();
        let client = Rc::clone(&self.inner);

        future_to_promise(async move {
            let response = client
                .get(&url)
                .await
                .map_err(|e| JsValue::from_str(&format!("{e}")))?;

            Ok(JsValue::from(to_wasm_response(response).await?))
        })
    }

    /// Send a POST request with an optional JSON body.
    ///
    /// Returns a `Promise<WasmL402Response>`.
    pub fn post(&self, url: &str, body: Option<String>) -> js_sys::Promise {
        let url = url.to_string();
        let client = Rc::clone(&self.inner);

        future_to_promise(async move {
            let response = client
                .post(&url, body.as_deref())
                .await
                .map_err(|e| JsValue::from_str(&format!("{e}")))?;

            Ok(JsValue::from(to_wasm_response(response).await?))
        })
    }

    /// Get the total amount spent in satoshis.
    #[wasm_bindgen(getter, js_name = "totalSpent")]
    pub fn total_spent(&self) -> js_sys::Promise {
        let client = Rc::clone(&self.inner);

        future_to_promise(async move {
            let spent = client.total_spent().await;
            Ok(JsValue::from_f64(spent as f64))
        })
    }

    /// Get all payment receipts.
    pub fn receipts(&self) -> js_sys::Promise {
        let client = Rc::clone(&self.inner);

        future_to_promise(async move {
            let receipts = client.receipts().await;
            let arr = js_sys::Array::new();
            for r in receipts {
                arr.push(&JsValue::from(WasmReceipt {
                    timestamp: r.timestamp,
                    endpoint: r.endpoint.clone(),
                    amount_sats: r.amount_sats,
                    fee_sats: r.fee_sats,
                    payment_hash: r.payment_hash.clone(),
                    preimage: r.preimage.clone(),
                    response_status: r.response_status,
                    latency_ms: r.latency_ms,
                }));
            }
            Ok(arr.into())
        })
    }
}

/// Convert an [`L402Response`] into a [`WasmL402Response`].
async fn to_wasm_response(
    response: bolt402_core::L402Response,
) -> Result<WasmL402Response, JsValue> {
    let paid = response.paid();
    let cached_token = response.cached_token();
    let receipt = response.receipt().map(|r| WasmReceipt {
        timestamp: r.timestamp,
        endpoint: r.endpoint.clone(),
        amount_sats: r.amount_sats,
        fee_sats: r.fee_sats,
        payment_hash: r.payment_hash.clone(),
        preimage: r.preimage.clone(),
        response_status: r.response_status,
        latency_ms: r.latency_ms,
    });
    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|e| JsValue::from_str(&format!("{e}")))?;

    Ok(WasmL402Response {
        status,
        paid,
        cached_token,
        body,
        receipt,
    })
}
