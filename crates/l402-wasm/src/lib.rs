//! WebAssembly bindings for the L402sdk L402 client SDK.
//!
//! Exposes the Rust L402 engine to JavaScript/TypeScript via `wasm-bindgen`,
//! enabling browser-based and edge-runtime AI agents to use L402-gated APIs
//! with Lightning payments.
//!
//! # Architecture
//!
//! The WASM module wraps the Rust `L402Client` from `l402-core`, providing
//! the full L402 protocol engine (challenge parsing, budget enforcement, token
//! caching, receipt tracking) compiled to WebAssembly. All protocol logic runs
//! in Rust — no TypeScript reimplementation needed.
//!
//! # Example (JavaScript)
//!
//! ```javascript
//! import init, { WasmL402Client, WasmBudgetConfig } from 'l402-wasm';
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

use wasm_bindgen::prelude::*;

/// Real Lightning backend wrappers (LND REST, CLN REST, SwissKnife).
pub mod backends;

pub use backends::{
    WasmClnRestBackend, WasmLndRestBackend, WasmNodeInfo, WasmPaymentResult, WasmSwissKnifeBackend,
};

/// L402 client wrapper (full protocol engine from l402-core).
pub mod client;

pub use client::{WasmBudgetConfig, WasmL402Client, WasmL402Response, WasmReceipt};

/// Install a panic hook that logs the panic message to `console.error`.
///
/// Call this once before using any other WASM functions to get human-readable
/// Rust panic messages instead of opaque `RuntimeError: unreachable`.
#[wasm_bindgen(js_name = "setPanicHook")]
pub fn set_panic_hook() {
    console_error_panic_hook::set_once();
}
