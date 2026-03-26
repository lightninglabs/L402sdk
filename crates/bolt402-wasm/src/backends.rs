//! WASM-bindgen wrappers for the real Lightning backends.
//!
//! Exposes `LndRestBackend`, `ClnRestBackend`, and `SwissKnifeBackend` from
//! Rust to JavaScript via wasm-bindgen, using proper typed structs.

use wasm_bindgen::prelude::*;

use bolt402_cln::ClnRestBackend;
use bolt402_lnd::LndRestBackend;
use bolt402_proto::LnBackend;
use bolt402_swissknife::SwissKnifeBackend as RustSwissKnifeBackend;

// ---------------------------------------------------------------------------
// Shared WASM types
// ---------------------------------------------------------------------------

/// Result of a Lightning payment, returned from `payInvoice`.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct WasmPaymentResult {
    preimage: String,
    payment_hash: String,
    /// Amount paid in satoshis (excluding routing fees).
    #[wasm_bindgen(readonly, js_name = "amountSats")]
    pub amount_sats: u64,
    /// Routing fee paid in satoshis.
    #[wasm_bindgen(readonly, js_name = "feeSats")]
    pub fee_sats: u64,
}

#[wasm_bindgen]
impl WasmPaymentResult {
    /// Hex-encoded payment preimage (proof of payment).
    #[wasm_bindgen(getter)]
    pub fn preimage(&self) -> String {
        self.preimage.clone()
    }

    /// Hex-encoded payment hash.
    #[wasm_bindgen(getter, js_name = "paymentHash")]
    pub fn payment_hash(&self) -> String {
        self.payment_hash.clone()
    }

    /// Total cost (amount + fee) in satoshis.
    #[wasm_bindgen(js_name = "totalCostSats")]
    pub fn total_cost_sats(&self) -> u64 {
        self.amount_sats + self.fee_sats
    }
}

/// Information about a Lightning node, returned from `getInfo`.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct WasmNodeInfo {
    pubkey: String,
    alias: String,
    /// Number of active channels.
    #[wasm_bindgen(readonly, js_name = "numActiveChannels")]
    pub num_active_channels: u32,
}

#[wasm_bindgen]
impl WasmNodeInfo {
    /// Node public key (hex-encoded).
    #[wasm_bindgen(getter)]
    pub fn pubkey(&self) -> String {
        self.pubkey.clone()
    }

    /// Node alias.
    #[wasm_bindgen(getter)]
    pub fn alias(&self) -> String {
        self.alias.clone()
    }
}

// ---------------------------------------------------------------------------
// WasmLndRestBackend
// ---------------------------------------------------------------------------

/// LND REST backend for use in JavaScript/TypeScript.
///
/// Wraps the Rust `LndRestBackend` which uses `reqwest` (compiled to
/// browser `fetch` on WASM).
///
/// # Example
///
/// ```javascript
/// import init, { WasmLndRestBackend } from 'bolt402-wasm';
///
/// await init();
///
/// const lnd = new WasmLndRestBackend("https://localhost:8080", "deadbeef...");
/// const info = await lnd.getInfo();
/// console.log(info.alias);
/// ```
#[wasm_bindgen]
pub struct WasmLndRestBackend {
    inner: LndRestBackend,
}

#[wasm_bindgen]
impl WasmLndRestBackend {
    /// Create a new LND REST backend.
    ///
    /// # Arguments
    ///
    /// * `url` - LND REST API URL (e.g. `https://localhost:8080`)
    /// * `macaroon` - Hex-encoded admin macaroon
    #[wasm_bindgen(constructor)]
    pub fn new(url: &str, macaroon: &str) -> Result<WasmLndRestBackend, JsError> {
        let inner = LndRestBackend::new(url, macaroon)
            .map_err(|e| JsError::new(&format!("failed to create LND backend: {e}")))?;
        Ok(Self { inner })
    }

    /// Pay a BOLT11 Lightning invoice.
    #[wasm_bindgen(js_name = "payInvoice")]
    pub async fn pay_invoice(
        &self,
        bolt11: &str,
        max_fee_sats: u64,
    ) -> Result<WasmPaymentResult, JsError> {
        let result = self
            .inner
            .pay_invoice(bolt11, max_fee_sats)
            .await
            .map_err(|e| JsError::new(&format!("{e}")))?;

        Ok(WasmPaymentResult {
            preimage: result.preimage,
            payment_hash: result.payment_hash,
            amount_sats: result.amount_sats,
            fee_sats: result.fee_sats,
        })
    }

    /// Get the current spendable balance in satoshis.
    #[wasm_bindgen(js_name = "getBalance")]
    pub async fn get_balance(&self) -> Result<u64, JsError> {
        self.inner
            .get_balance()
            .await
            .map_err(|e| JsError::new(&format!("{e}")))
    }

    /// Get information about the connected Lightning node.
    #[wasm_bindgen(js_name = "getInfo")]
    pub async fn get_info(&self) -> Result<WasmNodeInfo, JsError> {
        let info = self
            .inner
            .get_info()
            .await
            .map_err(|e| JsError::new(&format!("{e}")))?;

        Ok(WasmNodeInfo {
            pubkey: info.pubkey,
            alias: info.alias,
            num_active_channels: info.num_active_channels,
        })
    }
}

// ---------------------------------------------------------------------------
// WasmClnRestBackend
// ---------------------------------------------------------------------------

/// CLN REST backend for use in JavaScript/TypeScript.
///
/// Wraps the Rust `ClnRestBackend` which uses `reqwest` (compiled to
/// browser `fetch` on WASM). Authenticates via rune token.
///
/// # Example
///
/// ```javascript
/// import init, { WasmClnRestBackend } from 'bolt402-wasm';
///
/// await init();
///
/// const cln = new WasmClnRestBackend("https://localhost:3001", "rune_token...");
/// const info = await cln.getInfo();
/// console.log(info.alias);
/// ```
#[wasm_bindgen]
pub struct WasmClnRestBackend {
    inner: ClnRestBackend,
}

#[wasm_bindgen]
impl WasmClnRestBackend {
    /// Create a new CLN REST backend using rune authentication.
    ///
    /// # Arguments
    ///
    /// * `url` - CLN REST API URL (e.g. `https://localhost:3001`)
    /// * `rune` - Rune token string
    #[wasm_bindgen(constructor)]
    pub fn new(url: &str, rune: &str) -> Result<WasmClnRestBackend, JsError> {
        let inner = ClnRestBackend::new(url, rune)
            .map_err(|e| JsError::new(&format!("failed to create CLN backend: {e}")))?;
        Ok(Self { inner })
    }

    /// Create a new CLN REST backend with rune authentication.
    ///
    /// Named constructor for clarity — equivalent to `new WasmClnRestBackend(url, rune)`.
    ///
    /// # Arguments
    ///
    /// * `url` - CLN REST API URL (e.g. `https://localhost:3001`)
    /// * `rune` - Rune token string (CLN's native bearer token)
    #[wasm_bindgen(js_name = "withRune")]
    pub fn with_rune(url: &str, rune: &str) -> Result<WasmClnRestBackend, JsError> {
        Self::new(url, rune)
    }

    /// Pay a BOLT11 Lightning invoice.
    #[wasm_bindgen(js_name = "payInvoice")]
    pub async fn pay_invoice(
        &self,
        bolt11: &str,
        max_fee_sats: u64,
    ) -> Result<WasmPaymentResult, JsError> {
        let result = self
            .inner
            .pay_invoice(bolt11, max_fee_sats)
            .await
            .map_err(|e| JsError::new(&format!("{e}")))?;

        Ok(WasmPaymentResult {
            preimage: result.preimage,
            payment_hash: result.payment_hash,
            amount_sats: result.amount_sats,
            fee_sats: result.fee_sats,
        })
    }

    /// Get the current spendable balance in satoshis.
    #[wasm_bindgen(js_name = "getBalance")]
    pub async fn get_balance(&self) -> Result<u64, JsError> {
        self.inner
            .get_balance()
            .await
            .map_err(|e| JsError::new(&format!("{e}")))
    }

    /// Get information about the connected Lightning node.
    #[wasm_bindgen(js_name = "getInfo")]
    pub async fn get_info(&self) -> Result<WasmNodeInfo, JsError> {
        let info = self
            .inner
            .get_info()
            .await
            .map_err(|e| JsError::new(&format!("{e}")))?;

        Ok(WasmNodeInfo {
            pubkey: info.pubkey,
            alias: info.alias,
            num_active_channels: info.num_active_channels,
        })
    }
}

// ---------------------------------------------------------------------------
// WasmSwissKnifeBackend
// ---------------------------------------------------------------------------

/// `SwissKnife` REST backend for use in JavaScript/TypeScript.
///
/// Wraps the Rust `SwissKnifeBackend` which uses `reqwest`.
///
/// # Example
///
/// ```javascript
/// import init, { WasmSwissKnifeBackend } from 'bolt402-wasm';
///
/// await init();
///
/// const sk = new WasmSwissKnifeBackend("https://app.numeraire.tech", "sk-...");
/// const info = await sk.getInfo();
/// ```
#[wasm_bindgen]
pub struct WasmSwissKnifeBackend {
    inner: RustSwissKnifeBackend,
}

#[wasm_bindgen]
impl WasmSwissKnifeBackend {
    /// Create a new `SwissKnife` backend.
    ///
    /// # Arguments
    ///
    /// * `url` - `SwissKnife` API URL (e.g. `https://app.numeraire.tech`)
    /// * `api_key` - API key for authentication
    #[wasm_bindgen(constructor)]
    pub fn new(url: &str, api_key: &str) -> Self {
        Self {
            inner: RustSwissKnifeBackend::new(url, api_key),
        }
    }

    /// Pay a BOLT11 Lightning invoice.
    #[wasm_bindgen(js_name = "payInvoice")]
    pub async fn pay_invoice(
        &self,
        bolt11: &str,
        max_fee_sats: u64,
    ) -> Result<WasmPaymentResult, JsError> {
        let result = self
            .inner
            .pay_invoice(bolt11, max_fee_sats)
            .await
            .map_err(|e| JsError::new(&format!("{e}")))?;

        Ok(WasmPaymentResult {
            preimage: result.preimage,
            payment_hash: result.payment_hash,
            amount_sats: result.amount_sats,
            fee_sats: result.fee_sats,
        })
    }

    /// Get the current spendable balance in satoshis.
    #[wasm_bindgen(js_name = "getBalance")]
    pub async fn get_balance(&self) -> Result<u64, JsError> {
        self.inner
            .get_balance()
            .await
            .map_err(|e| JsError::new(&format!("{e}")))
    }

    /// Get information about the connected Lightning node.
    #[wasm_bindgen(js_name = "getInfo")]
    pub async fn get_info(&self) -> Result<WasmNodeInfo, JsError> {
        let info = self
            .inner
            .get_info()
            .await
            .map_err(|e| JsError::new(&format!("{e}")))?;

        Ok(WasmNodeInfo {
            pubkey: info.pubkey,
            alias: info.alias,
            num_active_channels: info.num_active_channels,
        })
    }
}
