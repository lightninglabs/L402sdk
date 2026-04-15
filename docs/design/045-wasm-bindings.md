# Design Doc 045: WASM Bindings via wasm-pack

**Issue:** #45
**Date:** 2026-03-20
**Author:** Dario Anongba Varela
**Status:** Implemented

## Problem

Before the WASM work, `l402-ai-sdk` carried a pure TypeScript implementation that duplicated protocol logic from Rust. Browser-based and edge-runtime AI agents could not reuse the Rust core directly.

WASM bindings complete the cross-language story by enabling:

- **Browser AI agents** — L402 payments directly from the browser
- **Edge runtimes** — Cloudflare Workers, Deno Deploy, Vercel Edge Functions
- **Universal WASM runtimes** — Wasmtime, Wasmer, etc.

## Design

### Crate: `crates/l402-wasm/`

A wasm-bindgen wrapper that exposes both **real Lightning backends** (via Rust, compiled to WASM) and an **in-process mock** for testing/demos.

### Architecture

```
                  ┌─────────────────────┐
                  │   JavaScript/TS     │
                  │   (browser / Node)  │
                  └─────────┬───────────┘
                            │ wasm-bindgen
                  ┌─────────▼───────────┐
                  │    l402-wasm     │
                  │   WasmL402Client    │
                  │  WasmLndRestBackend │
                  │  WasmClnRestBackend │
                  │ WasmSwissKnifeBack. │
                  └─────────┬───────────┘
                            │
              ┌─────────────┼───────────────┐
              │             │               │
    ┌─────────▼──┐  ┌──────▼──────┐  ┌─────▼─────────┐
    │l402-proto│  │l402-lnd  │  │L402sdk-       │
    │(types,ports│  │(rest feature)│  │swissknife     │
    │ errors)    │  └─────────────┘  └───────────────┘
    └────────────┘
```

**Key insight:** `l402-core` is WASM-safe after moving off a tokio runtime dependency, so `l402-wasm` can wrap the real Rust `L402Client` directly while still exposing standalone backend bindings. The REST backends use `reqwest`, which compiles to browser `fetch` on `wasm32-unknown-unknown`.

### Key Decisions

1. **wasm-bindgen + wasm-pack** — Standard toolchain. Auto-generates TypeScript type definitions. npm-publishable.

2. **No tokio in WASM path** — Port traits (`LnBackend`, `TokenStore`) and `ClientError` live in `l402-proto` (no async runtime dependency). Backend crates (`l402-lnd[rest]`, `l402-swissknife`) depend only on `l402-proto`. This was achieved by moving ports from `l402-core` to `l402-proto`.

3. **Real backends compiled to WASM** — `l402-lnd` (REST feature), `l402-cln` (REST feature), and `l402-swissknife` all use `reqwest`, which compiles to `wasm32-unknown-unknown` using browser `fetch`. No JS callback delegation needed. Wrapped as `WasmLndRestBackend`, `WasmClnRestBackend`, and `WasmSwissKnifeBackend`.

4. **Conditional async_trait** — Port traits use `#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]` because `reqwest::Response` is not `Send` on WASM targets.

5. **Conditional platform APIs** — `danger_accept_invalid_certs()` and `from_env()` are gated behind `#[cfg(not(target_arch = "wasm32"))]` since they don't apply in browsers.

6. **Rust L402 client first** — `WasmL402Client` wraps the real `l402-core::L402Client`, so challenge parsing, token caching, budget enforcement, and receipts all stay in one implementation.

7. **Budget in WASM** — Full budget enforcement (per-request, hourly, daily, total) works through the shared Rust `BudgetTracker`.

### API Surface

```typescript
// Full L402 client over LND REST
const client = WasmL402Client.withLndRest(
  "https://localhost:8080",
  "deadbeef...",
  WasmBudgetConfig.unlimited(),
  100,
);
const response = await client.get("https://api.example.com/data");

// Direct CLN REST backend
const cln = WasmClnRestBackend.withRune(
  "https://localhost:3010",
  "rune-token-value...",
);
const info = await cln.getInfo();

// Utilities
const { macaroon, invoice } = parseL402Challenge(headerValue);
const header = buildL402Header(macaroon, preimage);
```

### Crate Structure

```
crates/l402-wasm/
├── Cargo.toml
├── src/
│   ├── lib.rs          # Entry points and wasm-bindgen exports
│   ├── client.rs       # WasmL402Client, WasmBudgetConfig, receipts/responses
│   └── backends.rs     # WasmLndRestBackend, WasmClnRestBackend, WasmSwissKnifeBackend
├── tests/
│   └── web.rs          # wasm-pack test (headless browser)
└── README.md
```

### Dependencies

- `l402-proto` — Types, port traits, errors (WASM-safe)
- `l402-lnd` (default-features = false, features = ["rest"]) — LND REST backend
- `l402-swissknife` — SwissKnife REST backend
- `wasm-bindgen` — Core WASM-JS bridge
- `wasm-bindgen-futures` — async/Promise interop
- `js-sys` — JS standard library access
- `serde-wasm-bindgen` — Serde-JsValue conversion

### Testing Plan

- `cargo test -p l402-wasm` — Native unit tests (mock challenge generation, budget, etc.)
- `wasm-pack test --headless --chrome crates/l402-wasm` — Browser tests via wasm-bindgen-test
- CI: `wasm-pack build` + both test suites

## CI

The `wasm` job builds and tests the WASM bindings. The `typescript` job depends on `wasm` and builds `l402-wasm` before `yarn install` (since `l402-ai-sdk` depends on `l402-wasm@file:../../crates/l402-wasm/pkg`).

```yaml
wasm:
  name: WASM Bindings
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
      with:
        targets: wasm32-unknown-unknown
    - uses: Swatinem/rust-cache@v2
    - name: Install wasm-pack
      run: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
    - name: Build WASM (web target)
      run: wasm-pack build crates/l402-wasm --target web
    - name: Run native unit tests
      run: cargo test -p l402-wasm
    - name: Run WASM browser tests
      run: wasm-pack test --headless --chrome crates/l402-wasm

typescript:
  name: TypeScript (l402-ai-sdk)
  needs: wasm
  runs-on: ubuntu-latest
  steps:
    # ... builds WASM first, then yarn install + tsc + vitest
```
