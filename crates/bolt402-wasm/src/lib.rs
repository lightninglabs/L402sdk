//! WebAssembly bindings for the bolt402 L402 client SDK.
//!
//! Exposes the Rust L402 engine to JavaScript/TypeScript via `wasm-bindgen`,
//! enabling browser-based and edge-runtime AI agents to use L402-gated APIs
//! with Lightning payments.
//!
//! # Architecture
//!
//! The WASM module wraps the Rust `L402Client` from `bolt402-core`, providing
//! the full L402 protocol engine (challenge parsing, budget enforcement, token
//! caching, receipt tracking) compiled to WebAssembly. All protocol logic runs
//! in Rust — no TypeScript reimplementation needed.
//!
//! Additionally, an in-process mock L402 environment is provided for testing,
//! demos, and development without a real Lightning node.
//!
//! # Example — Real L402 Client (JavaScript)
//!
//! ```javascript
//! import init, { WasmL402Client, WasmBudgetConfig } from 'bolt402-wasm';
//!
//! await init();
//!
//! const client = WasmL402Client.withLndRest(
//!   "https://localhost:8080",
//!   "deadbeef...",
//!   WasmBudgetConfig.unlimited(),
//!   100,
//! );
//!
//! const response = await client.get("https://api.example.com/data");
//! console.log(response.status, response.paid, response.body);
//! ```
//!
//! # Example — Mock (JavaScript)
//!
//! ```javascript
//! import init, { WasmMockServer, WasmMockClient } from 'bolt402-wasm';
//!
//! await init();
//!
//! const server = new WasmMockServer({ "/api/data": 10n });
//! const client = new WasmMockClient(server, 100n);
//! const response = client.get("/api/data");
//!
//! console.log(response.status);    // 200
//! console.log(response.paid);      // true
//! console.log(response.receipt);   // { amountSats: 10, ... }
//! ```

/// Real Lightning backend wrappers (LND REST, SwissKnife).
pub mod backends;

/// L402 client wrapper (full protocol engine from bolt402-core).
pub mod client;

/// Install a panic hook that logs the panic message to `console.error`.
///
/// Call this once before using any other WASM functions to get human-readable
/// Rust panic messages instead of opaque `RuntimeError: unreachable`.
#[wasm_bindgen(js_name = "setPanicHook")]
pub fn set_panic_hook() {
    console_error_panic_hook::set_once();
}

use std::cell::RefCell;
use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;

use bolt402_proto::{L402Challenge, L402Token};

// ---------------------------------------------------------------------------
// Internal: Challenge generation (WASM-safe, no /dev/urandom)
// ---------------------------------------------------------------------------

/// A pending L402 challenge for the in-process mock.
#[derive(Debug, Clone)]
struct MockChallenge {
    preimage: String,
    payment_hash: String,
    macaroon: String,
    invoice: String,
    amount_sats: u64,
}

impl MockChallenge {
    fn generate(amount_sats: u64) -> Self {
        let preimage_bytes = wasm_rand_bytes();
        let preimage = hex::encode(preimage_bytes);

        let mut hasher = Sha256::new();
        hasher.update(preimage_bytes);
        let hash_bytes = hasher.finalize();
        let payment_hash = hex::encode(hash_bytes);

        let macaroon_data = format!(r#"{{"payment_hash":"{payment_hash}"}}"#);
        let macaroon = BASE64.encode(macaroon_data.as_bytes());

        // Use bech32-safe characters (no '1', 'b', 'i', 'o') for the data portion
        let safe_hash: String = payment_hash
            .chars()
            .map(|c| if c == '1' { 'x' } else { c })
            .take(20)
            .collect();

        let invoice = if amount_sats >= 100 && amount_sats % 100 == 0 {
            format!("lnbc{}u1mock{safe_hash}", amount_sats / 100)
        } else {
            format!("lnbc{}n1mock{safe_hash}", amount_sats * 10)
        };

        Self {
            preimage,
            payment_hash,
            macaroon,
            invoice,
            amount_sats,
        }
    }

    fn to_www_authenticate(&self) -> String {
        format!(
            r#"L402 macaroon="{}", invoice="{}""#,
            self.macaroon, self.invoice
        )
    }

    fn validate_preimage(&self, preimage_hex: &str) -> bool {
        let Ok(preimage_bytes) = hex::decode(preimage_hex) else {
            return false;
        };
        let mut hasher = Sha256::new();
        hasher.update(&preimage_bytes);
        hex::encode(hasher.finalize()) == self.payment_hash
    }

    fn validate_auth(&self, macaroon: &str, preimage_hex: &str) -> bool {
        macaroon == self.macaroon && self.validate_preimage(preimage_hex)
    }
}

/// Generate random bytes.
///
/// Uses `js_sys::Math::random` on WASM targets and `/dev/urandom` (with
/// timestamp fallback) on native targets (for unit tests).
fn wasm_rand_bytes() -> [u8; 32] {
    let mut buf = [0u8; 32];

    #[cfg(target_arch = "wasm32")]
    {
        for byte in &mut buf {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                *byte = (js_sys::Math::random() * 256.0) as u8;
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(unix)]
        {
            use std::io::Read;
            if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
                let _ = f.read_exact(&mut buf);
                return buf;
            }
        }
        // Timestamp fallback (fine for tests)
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_nanos();
        #[allow(clippy::cast_possible_truncation)]
        for (i, byte) in buf.iter_mut().enumerate() {
            let shift_a = i % 16;
            let shift_b = (i + 7) % 16;
            *byte = ((seed >> shift_a) ^ (seed >> shift_b)) as u8;
        }
    }

    buf
}

/// Current time as Unix timestamp (seconds).
///
/// Uses `js_sys::Date::now` on WASM targets and `SystemTime` on native targets.
fn now_secs() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            (js_sys::Date::now() / 1000.0) as u64
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_secs()
    }
}

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

    /// Routing fee in satoshis (always 0 for mock).
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

// ---------------------------------------------------------------------------
// WasmResponse
// ---------------------------------------------------------------------------

/// Response from an L402-aware request.
///
/// Contains the HTTP status, whether a payment was made, the response body,
/// and an optional payment receipt.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct WasmResponse {
    /// HTTP status code.
    #[wasm_bindgen(readonly)]
    pub status: u16,

    /// Whether a Lightning payment was made.
    #[wasm_bindgen(readonly)]
    pub paid: bool,

    /// Response body.
    body: String,

    /// Payment receipt (if a payment was made).
    receipt: Option<WasmReceipt>,
}

#[wasm_bindgen]
impl WasmResponse {
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
// WasmBudget
// ---------------------------------------------------------------------------

/// Budget configuration for limiting L402 payments.
///
/// Prevents runaway spending by enforcing caps at multiple granularities.
#[wasm_bindgen]
#[derive(Debug, Clone)]
#[allow(clippy::struct_field_names)]
pub struct WasmBudget {
    per_request_max: Option<u64>,
    hourly_max: Option<u64>,
    daily_max: Option<u64>,
    total_max: Option<u64>,
}

#[wasm_bindgen]
impl WasmBudget {
    /// Create a new budget configuration.
    ///
    /// All limits are optional. Pass `0` (or omit) for no limit on that
    /// granularity. Amounts are in satoshis.
    #[wasm_bindgen(constructor)]
    pub fn new(
        per_request_max: Option<u64>,
        hourly_max: Option<u64>,
        daily_max: Option<u64>,
        total_max: Option<u64>,
    ) -> Self {
        Self {
            per_request_max,
            hourly_max,
            daily_max,
            total_max,
        }
    }

    /// Create an unlimited budget (no restrictions).
    pub fn unlimited() -> Self {
        Self {
            per_request_max: None,
            hourly_max: None,
            daily_max: None,
            total_max: None,
        }
    }

    /// Maximum per-request amount, or `undefined` for no limit.
    #[wasm_bindgen(getter, js_name = "perRequestMax")]
    pub fn per_request_max(&self) -> Option<u64> {
        self.per_request_max
    }

    /// Maximum hourly amount, or `undefined` for no limit.
    #[wasm_bindgen(getter, js_name = "hourlyMax")]
    pub fn hourly_max(&self) -> Option<u64> {
        self.hourly_max
    }

    /// Maximum daily amount, or `undefined` for no limit.
    #[wasm_bindgen(getter, js_name = "dailyMax")]
    pub fn daily_max(&self) -> Option<u64> {
        self.daily_max
    }

    /// Maximum total amount, or `undefined` for no limit.
    #[wasm_bindgen(getter, js_name = "totalMax")]
    pub fn total_max(&self) -> Option<u64> {
        self.total_max
    }
}

// ---------------------------------------------------------------------------
// Internal: Budget tracker state
// ---------------------------------------------------------------------------

/// Tracks spending over time windows for budget enforcement.
#[derive(Debug, Default)]
struct BudgetState {
    total: u64,
    /// Spending per hour (keyed by `unix_secs / 3600`).
    hourly: HashMap<u64, u64>,
    /// Spending per day (keyed by `unix_secs / 86400`).
    daily: HashMap<u64, u64>,
}

impl BudgetState {
    fn check_and_record(&mut self, budget: &WasmBudget, amount: u64) -> Result<(), String> {
        // Per-request check
        if let Some(max) = budget.per_request_max {
            if amount > max {
                return Err(format!(
                    "payment of {amount} sats exceeds per-request limit of {max} sats"
                ));
            }
        }

        let now = now_secs();
        let current_hour = now / 3600;
        let current_day = now / 86400;

        // Hourly check
        if let Some(hourly_max) = budget.hourly_max {
            let hourly_spent = self.hourly.get(&current_hour).copied().unwrap_or(0);
            if hourly_spent + amount > hourly_max {
                return Err(format!(
                    "payment of {amount} sats would exceed hourly limit ({hourly_spent} + {amount} > {hourly_max})"
                ));
            }
        }

        // Daily check
        if let Some(daily_max) = budget.daily_max {
            let daily_spent = self.daily.get(&current_day).copied().unwrap_or(0);
            if daily_spent + amount > daily_max {
                return Err(format!(
                    "payment of {amount} sats would exceed daily limit ({daily_spent} + {amount} > {daily_max})"
                ));
            }
        }

        // Total check
        if let Some(total_max) = budget.total_max {
            if self.total + amount > total_max {
                return Err(format!(
                    "payment of {amount} sats would exceed total limit ({} + {amount} > {total_max})",
                    self.total
                ));
            }
        }

        // Record
        self.total += amount;
        *self.hourly.entry(current_hour).or_insert(0) += amount;
        *self.daily.entry(current_day).or_insert(0) += amount;

        // Prune old entries
        let cutoff_hour = current_hour.saturating_sub(48);
        self.hourly.retain(|&k, _| k >= cutoff_hour);
        let cutoff_day = current_day.saturating_sub(2);
        self.daily.retain(|&k, _| k >= cutoff_day);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// WasmMockServer
// ---------------------------------------------------------------------------

/// In-process mock L402 server for testing in WASM environments.
///
/// Simulates L402 challenge-response authentication without any HTTP server.
/// Endpoints are configured as path → price (satoshis) pairs.
///
/// # Example
///
/// ```javascript
/// const server = new WasmMockServer({ "/api/data": 10n, "/api/premium": 100n });
/// ```
#[wasm_bindgen]
pub struct WasmMockServer {
    /// Endpoint configurations: path → (`price_sats`, `response_body`).
    endpoints: HashMap<String, (u64, String)>,
    /// Pending challenges keyed by invoice string.
    challenges: RefCell<HashMap<String, MockChallenge>>,
    /// Simulated wallet balance in satoshis.
    balance: RefCell<u64>,
}

#[wasm_bindgen]
impl WasmMockServer {
    /// Create a new mock server with the given endpoint configuration.
    ///
    /// `endpoints` is a JavaScript object mapping paths to prices:
    /// `{ "/api/data": 10, "/api/premium": 100 }`.
    ///
    /// Prices can be numbers or `BigInt` values.
    #[wasm_bindgen(constructor)]
    pub fn new(endpoints: JsValue) -> Result<WasmMockServer, JsError> {
        let map: HashMap<String, u64> = serde_wasm_bindgen::from_value(endpoints)
            .map_err(|e| JsError::new(&format!("invalid endpoints: {e}")))?;

        let endpoint_configs: HashMap<String, (u64, String)> = map
            .into_iter()
            .map(|(path, price)| {
                let body = format!(r#"{{"ok":true,"price":{price}}}"#);
                (path, (price, body))
            })
            .collect();

        Ok(Self {
            endpoints: endpoint_configs,
            challenges: RefCell::new(HashMap::new()),
            balance: RefCell::new(1_000_000),
        })
    }

    /// Get the list of configured endpoint paths.
    #[wasm_bindgen(js_name = "endpointPaths")]
    pub fn endpoint_paths(&self) -> Vec<JsValue> {
        self.endpoints
            .keys()
            .map(|k| JsValue::from_str(k))
            .collect()
    }

    /// Get the current simulated wallet balance in satoshis.
    #[wasm_bindgen(getter)]
    pub fn balance(&self) -> u64 {
        *self.balance.borrow()
    }

    /// Set the simulated wallet balance (for testing insufficient funds).
    #[wasm_bindgen(setter)]
    pub fn set_balance(&self, sats: u64) {
        *self.balance.borrow_mut() = sats;
    }
}

impl WasmMockServer {
    /// Handle a request to a path, returning (status, body, optional challenge).
    fn handle_request(
        &self,
        path: &str,
        auth: Option<(&str, &str)>,
    ) -> (u16, String, Option<MockChallenge>) {
        let Some((price_sats, response_body)) = self.endpoints.get(path) else {
            return (404, "not found".to_string(), None);
        };

        // Check existing authorization
        if let Some((macaroon, preimage)) = auth {
            let challenges = self.challenges.borrow();
            for challenge in challenges.values() {
                if challenge.validate_auth(macaroon, preimage) {
                    return (200, response_body.clone(), None);
                }
            }
            return (401, "invalid L402 token".to_string(), None);
        }

        // Issue a new challenge
        let challenge = MockChallenge::generate(*price_sats);
        self.challenges
            .borrow_mut()
            .insert(challenge.invoice.clone(), challenge.clone());

        (402, "payment required".to_string(), Some(challenge))
    }

    /// Pay an invoice by looking up the preimage from pending challenges.
    fn pay_invoice(&self, invoice: &str) -> Result<(String, String, u64), String> {
        let challenges = self.challenges.borrow();
        let challenge = challenges
            .get(invoice)
            .ok_or_else(|| format!("unknown invoice: {invoice}"))?;

        let mut balance = self.balance.borrow_mut();
        if *balance < challenge.amount_sats {
            return Err(format!(
                "insufficient balance: have {} sats, need {}",
                *balance, challenge.amount_sats
            ));
        }
        *balance -= challenge.amount_sats;

        Ok((
            challenge.preimage.clone(),
            challenge.payment_hash.clone(),
            challenge.amount_sats,
        ))
    }
}

// ---------------------------------------------------------------------------
// WasmMockClient
// ---------------------------------------------------------------------------

/// In-process L402 client for the WASM mock environment.
///
/// Connects to a [`WasmMockServer`] and executes the full L402 protocol flow
/// in-process (no HTTP). Supports budget enforcement, token caching, and
/// receipt tracking.
///
/// # Example
///
/// ```javascript
/// const server = new WasmMockServer({ "/api/data": 10n });
/// const client = new WasmMockClient(server, 100n);
///
/// const resp = client.get("/api/data");
/// console.log(resp.status);   // 200
/// console.log(resp.paid);     // true
///
/// console.log(client.totalSpent);  // 10n
/// ```
#[wasm_bindgen]
pub struct WasmMockClient {
    server: WasmMockServer,
    max_fee_sats: u64,
    budget: WasmBudget,
    budget_state: RefCell<BudgetState>,
    token_cache: RefCell<HashMap<String, (String, String)>>,
    receipts: RefCell<Vec<WasmReceipt>>,
}

#[wasm_bindgen]
impl WasmMockClient {
    /// Create a new mock client connected to the given server.
    ///
    /// # Arguments
    ///
    /// * `server` - The mock server to connect to (ownership is transferred)
    /// * `max_fee_sats` - Maximum routing fee in satoshis (for budget accounting)
    #[wasm_bindgen(constructor)]
    pub fn new(server: WasmMockServer, max_fee_sats: u64) -> Self {
        Self {
            server,
            max_fee_sats,
            budget: WasmBudget::unlimited(),
            budget_state: RefCell::new(BudgetState::default()),
            token_cache: RefCell::new(HashMap::new()),
            receipts: RefCell::new(Vec::new()),
        }
    }

    /// Create a new mock client with a budget.
    #[wasm_bindgen(js_name = "withBudget")]
    pub fn with_budget(server: WasmMockServer, max_fee_sats: u64, budget: WasmBudget) -> Self {
        Self {
            server,
            max_fee_sats,
            budget,
            budget_state: RefCell::new(BudgetState::default()),
            token_cache: RefCell::new(HashMap::new()),
            receipts: RefCell::new(Vec::new()),
        }
    }

    /// Send a GET request to a path, handling L402 payment automatically.
    pub fn get(&self, path: &str) -> Result<WasmResponse, JsError> {
        self.request(path)
    }

    /// Send a POST request to a path, handling L402 payment automatically.
    ///
    /// Note: In the mock environment, the body is ignored (responses are
    /// pre-configured). This method exists for API parity with the real client.
    pub fn post(&self, path: &str) -> Result<WasmResponse, JsError> {
        self.request(path)
    }

    /// Get the total amount spent in satoshis.
    #[wasm_bindgen(getter, js_name = "totalSpent")]
    pub fn total_spent(&self) -> u64 {
        self.budget_state.borrow().total
    }

    /// Get the number of payments made.
    #[wasm_bindgen(getter, js_name = "paymentCount")]
    pub fn payment_count(&self) -> usize {
        self.receipts.borrow().len()
    }

    /// Get all payment receipts.
    pub fn receipts(&self) -> Vec<WasmReceipt> {
        self.receipts.borrow().clone()
    }

    /// Get the current server balance in satoshis.
    #[wasm_bindgen(getter, js_name = "serverBalance")]
    pub fn server_balance(&self) -> u64 {
        self.server.balance()
    }

    /// Get the maximum fee in satoshis.
    #[wasm_bindgen(getter, js_name = "maxFeeSats")]
    pub fn max_fee_sats(&self) -> u64 {
        self.max_fee_sats
    }

    /// Clear the token cache.
    #[wasm_bindgen(js_name = "clearCache")]
    pub fn clear_cache(&self) {
        self.token_cache.borrow_mut().clear();
    }
}

impl WasmMockClient {
    /// Core request handler implementing the L402 protocol flow.
    fn request(&self, path: &str) -> Result<WasmResponse, JsError> {
        // 1. Check token cache
        {
            let cache = self.token_cache.borrow();
            if let Some((macaroon, preimage)) = cache.get(path) {
                let (status, body, _) =
                    self.server.handle_request(path, Some((macaroon, preimage)));
                if status != 402 {
                    return Ok(WasmResponse {
                        status,
                        paid: false,
                        body,
                        receipt: None,
                    });
                }
                // Token rejected, fall through
                drop(cache);
                self.token_cache.borrow_mut().remove(path);
            }
        }

        // 2. Initial request without auth
        let (status, body, challenge) = self.server.handle_request(path, None);

        if status != 402 {
            return Ok(WasmResponse {
                status,
                paid: false,
                body,
                receipt: None,
            });
        }

        // 3. We got a 402 — parse the challenge
        let challenge = challenge.ok_or_else(|| JsError::new("402 without challenge"))?;
        let www_auth = challenge.to_www_authenticate();
        let parsed = L402Challenge::from_header(&www_auth)
            .map_err(|e| JsError::new(&format!("failed to parse L402 challenge: {e}")))?;

        // 4. Check budget
        self.budget_state
            .borrow_mut()
            .check_and_record(&self.budget, challenge.amount_sats)
            .map_err(|e| JsError::new(&e))?;

        // 5. Pay the invoice
        let (preimage, payment_hash, amount_sats) = self
            .server
            .pay_invoice(&parsed.invoice)
            .map_err(|e| JsError::new(&e))?;

        // 6. Cache the token
        self.token_cache.borrow_mut().insert(
            path.to_string(),
            (parsed.macaroon.clone(), preimage.clone()),
        );

        // 7. Retry with auth
        let token = L402Token::new(parsed.macaroon, preimage.clone());
        let auth_header = token.to_header_value();
        let parts: Vec<&str> = auth_header
            .strip_prefix("L402 ")
            .unwrap_or(&auth_header)
            .splitn(2, ':')
            .collect();

        let (retry_status, retry_body, _) = if parts.len() == 2 {
            self.server.handle_request(path, Some((parts[0], parts[1])))
        } else {
            return Err(JsError::new("malformed L402 token"));
        };

        // 8. Record receipt
        let receipt = WasmReceipt {
            timestamp: now_secs(),
            endpoint: path.to_string(),
            amount_sats,
            fee_sats: 0,
            payment_hash,
            preimage,
            response_status: retry_status,
            latency_ms: 0, // mock has no real latency tracking
        };

        self.receipts.borrow_mut().push(receipt.clone());

        Ok(WasmResponse {
            status: retry_status,
            paid: true,
            body: retry_body,
            receipt: Some(receipt),
        })
    }
}

// ---------------------------------------------------------------------------
// Standalone utility exports
// ---------------------------------------------------------------------------

/// Parse an L402 `WWW-Authenticate` header and return the macaroon and invoice.
///
/// Useful for manual L402 protocol handling in JavaScript.
///
/// # Example
///
/// ```javascript
/// const { macaroon, invoice } = parseL402Challenge(headerValue);
/// ```
#[wasm_bindgen(js_name = "parseL402Challenge")]
pub fn parse_l402_challenge(header: &str) -> Result<JsValue, JsError> {
    let challenge = L402Challenge::from_header(header)
        .map_err(|e| JsError::new(&format!("failed to parse L402 challenge: {e}")))?;

    let obj = js_sys::Object::new();
    js_sys::Reflect::set(
        &obj,
        &JsValue::from_str("macaroon"),
        &JsValue::from_str(&challenge.macaroon),
    )
    .map_err(|_| JsError::new("failed to set macaroon"))?;
    js_sys::Reflect::set(
        &obj,
        &JsValue::from_str("invoice"),
        &JsValue::from_str(&challenge.invoice),
    )
    .map_err(|_| JsError::new("failed to set invoice"))?;

    Ok(obj.into())
}

/// Construct an L402 `Authorization` header value.
///
/// Returns a string like `"L402 <macaroon>:<preimage>"`.
///
/// # Example
///
/// ```javascript
/// const header = buildL402Header(macaroon, preimage);
/// // "L402 YWJjZGVm:abcdef1234567890"
/// ```
#[wasm_bindgen(js_name = "buildL402Header")]
pub fn build_l402_header(macaroon: &str, preimage: &str) -> String {
    let token = L402Token::new(macaroon.to_string(), preimage.to_string());
    token.to_header_value()
}

/// Get the bolt402-wasm version string.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ---------------------------------------------------------------------------
// Tests (native, not WASM — see tests/web.rs for wasm_bindgen_test)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_generate_and_validate() {
        let challenge = MockChallenge::generate(100);
        assert!(challenge.validate_preimage(&challenge.preimage));
        assert!(challenge.validate_auth(&challenge.macaroon, &challenge.preimage));
    }

    #[test]
    fn challenge_reject_wrong_preimage() {
        let challenge = MockChallenge::generate(100);
        let fake = "0".repeat(64);
        assert!(!challenge.validate_preimage(&fake));
    }

    #[test]
    fn challenge_www_authenticate_format() {
        let challenge = MockChallenge::generate(100);
        let header = challenge.to_www_authenticate();
        assert!(header.starts_with("L402 macaroon=\""));
        assert!(header.contains("invoice=\"lnbc"));
    }

    #[test]
    fn budget_unlimited() {
        let budget = WasmBudget::unlimited();
        assert!(budget.per_request_max.is_none());
        assert!(budget.hourly_max.is_none());
        assert!(budget.daily_max.is_none());
        assert!(budget.total_max.is_none());
    }

    #[test]
    fn budget_per_request_check() {
        let budget = WasmBudget::new(Some(100), None, None, None);
        let mut state = BudgetState::default();

        assert!(state.check_and_record(&budget, 50).is_ok());
        assert!(state.check_and_record(&budget, 100).is_ok());
        assert!(state.check_and_record(&budget, 101).is_err());
    }

    #[test]
    fn budget_total_check() {
        let budget = WasmBudget::new(None, None, None, Some(500));
        let mut state = BudgetState::default();

        for _ in 0..5 {
            assert!(state.check_and_record(&budget, 100).is_ok());
        }
        assert!(state.check_and_record(&budget, 1).is_err());
        assert_eq!(state.total, 500);
    }

    #[test]
    fn build_header() {
        let header = build_l402_header("YWJjZGVm", "abcdef1234567890");
        assert_eq!(header, "L402 YWJjZGVm:abcdef1234567890");
    }

    #[test]
    fn version_not_empty() {
        assert!(!version().is_empty());
    }
}
