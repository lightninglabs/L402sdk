//! WASM-bindgen wrapper for the Rust [`l402_core::L402Client`] from `l402-core`.
//!
//! Exposes the full L402 protocol engine to JavaScript/TypeScript via
//! `wasm-bindgen`. The client handles HTTP 402 challenges, Lightning
//! payments, token caching, budget enforcement, and receipt tracking
//! entirely in Rust compiled to WASM.

use std::rc::Rc;

use wasm_bindgen::prelude::*;

use l402_core::budget::Budget;
use l402_core::cache::InMemoryTokenStore;
use l402_core::{L402Client, L402ClientConfig};
use l402_lnd::LndRestBackend;
use l402_swissknife::SwissKnifeBackend;

// ---------------------------------------------------------------------------
// WasmReceipt
// ---------------------------------------------------------------------------

/// A payment receipt from an L402 transaction.
///
/// Contains proof-of-payment data for audit and cost tracking.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct WasmReceipt {
    /// Unix timestamp (seconds) of the payment.
    #[wasm_bindgen(readonly, js_name = "timestamp")]
    pub timestamp: u64,

    /// Amount paid in satoshis (excluding routing fees).
    #[wasm_bindgen(readonly, js_name = "amountSats")]
    pub amount_sats: u64,

    /// Routing fee in satoshis.
    #[wasm_bindgen(readonly, js_name = "feeSats")]
    pub fee_sats: u64,

    /// HTTP status code of the final response.
    #[wasm_bindgen(readonly, js_name = "responseStatus")]
    pub response_status: u16,

    /// Total latency from initial request to final response (milliseconds).
    #[wasm_bindgen(readonly, js_name = "latencyMs")]
    pub latency_ms: u64,

    /// Endpoint path that was accessed.
    endpoint: String,

    /// Hex-encoded payment hash.
    payment_hash: String,

    /// Hex-encoded preimage (proof of payment).
    preimage: String,
}

#[wasm_bindgen]
impl WasmReceipt {
    /// The endpoint path that was accessed.
    #[wasm_bindgen(getter)]
    pub fn endpoint(&self) -> String {
        self.endpoint.clone()
    }

    /// Hex-encoded payment hash.
    #[wasm_bindgen(getter, js_name = "paymentHash")]
    pub fn payment_hash(&self) -> String {
        self.payment_hash.clone()
    }

    /// Hex-encoded preimage (proof of payment).
    #[wasm_bindgen(getter)]
    pub fn preimage(&self) -> String {
        self.preimage.clone()
    }

    /// Total cost (amount + fee) in satoshis.
    #[wasm_bindgen(js_name = "totalCostSats")]
    pub fn total_cost_sats(&self) -> u64 {
        self.amount_sats + self.fee_sats
    }
}

impl From<&l402_core::receipt::Receipt> for WasmReceipt {
    fn from(r: &l402_core::receipt::Receipt) -> Self {
        Self {
            timestamp: r.timestamp,
            endpoint: r.endpoint.clone(),
            amount_sats: r.amount_sats,
            fee_sats: r.fee_sats,
            payment_hash: r.payment_hash.clone(),
            preimage: r.preimage.clone(),
            response_status: r.response_status,
            latency_ms: r.latency_ms,
        }
    }
}

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
/// Wraps the Rust `L402Client` from `l402-core`. All protocol logic
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

    /// Create an L402 client backed by `SwissKnife` REST.
    ///
    /// # Arguments
    ///
    /// * `url` - `SwissKnife` API URL (e.g. `https://api.numeraire.tech`)
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
    pub async fn get(&self, url: &str) -> Result<WasmL402Response, JsError> {
        let response = self
            .inner
            .get(url)
            .await
            .map_err(|e| JsError::new(&format!("{e}")))?;

        to_wasm_response(response).await
    }

    /// Send a POST request with an optional JSON body.
    pub async fn post(&self, url: &str, body: Option<String>) -> Result<WasmL402Response, JsError> {
        let response = self
            .inner
            .post(url, body.as_deref())
            .await
            .map_err(|e| JsError::new(&format!("{e}")))?;

        to_wasm_response(response).await
    }

    /// Get the total amount spent in satoshis.
    #[wasm_bindgen(js_name = "totalSpent")]
    pub async fn total_spent(&self) -> u64 {
        self.inner.total_spent().await
    }

    /// Get all payment receipts.
    pub async fn receipts(&self) -> Vec<WasmReceipt> {
        self.inner
            .receipts()
            .await
            .iter()
            .map(WasmReceipt::from)
            .collect()
    }
}

/// Convert an [`L402Response`] into a [`WasmL402Response`].
async fn to_wasm_response(response: l402_core::L402Response) -> Result<WasmL402Response, JsError> {
    let paid = response.paid();
    let cached_token = response.cached_token();
    let receipt = response.receipt().map(WasmReceipt::from);
    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|e| JsError::new(&format!("{e}")))?;

    Ok(WasmL402Response {
        status,
        paid,
        cached_token,
        body,
        receipt,
    })
}
