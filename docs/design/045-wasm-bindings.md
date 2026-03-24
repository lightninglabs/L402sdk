# Design Doc 045: WASM Bindings via wasm-pack

**Issue:** #45
**Date:** 2026-03-20
**Author:** Toshi (maintainer)
**Status:** Implemented

## Problem

bolt402 has native bindings for Python (PyO3), Go (CGo/FFI), and TypeScript (pure). However, the TypeScript package (`bolt402-ai-sdk`) was a pure TS implementation that duplicated protocol logic. Browser-based and edge-runtime AI agents cannot use the Rust core directly.

WASM bindings complete the cross-language story by enabling:

- **Browser AI agents** — L402 payments directly from the browser
- **Edge runtimes** — Cloudflare Workers, Deno Deploy, Vercel Edge Functions
- **Universal WASM runtimes** — Wasmtime, Wasmer, etc.

## Design

### Crate: `crates/bolt402-wasm/`

A wasm-bindgen wrapper that exposes both **real Lightning backends** (via Rust, compiled to WASM) and an **in-process mock** for testing/demos.

### Architecture

```
                  ┌─────────────────────┐
                  │   JavaScript/TS     │
                  │   (browser / Node)  │
                  └─────────┬───────────┘
                            │ wasm-bindgen
                  ┌─────────▼───────────┐
                  │    bolt402-wasm     │
                  │  WasmLndRestBackend │
                  │  WasmSwissKnife..  │
                  │  WasmMockServer    │
                  │  WasmMockClient    │
                  └─────────┬───────────┘
                            │
              ┌─────────────┼───────────────┐
              │             │               │
    ┌─────────▼──┐  ┌──────▼──────┐  ┌─────▼─────────┐
    │bolt402-proto│  │bolt402-lnd  │  │bolt402-       │
    │(types,ports│  │(rest feature)│  │swissknife     │
    │ errors)    │  └─────────────┘  └───────────────┘
    └────────────┘
```

**Key insight:** `bolt402-wasm` depends on `bolt402-proto` + backend crates directly, **not** `bolt402-core`. This avoids pulling in tokio (which `bolt402-core` uses for `RwLock`). The backends use `reqwest` which compiles to browser `fetch` on `wasm32-unknown-unknown`.

### Key Decisions

1. **wasm-bindgen + wasm-pack** — Standard toolchain. Auto-generates TypeScript type definitions. npm-publishable.

2. **No tokio in WASM path** — Port traits (`LnBackend`, `TokenStore`) and `ClientError` live in `bolt402-proto` (no async runtime dependency). Backend crates (`bolt402-lnd[rest]`, `bolt402-swissknife`) depend only on `bolt402-proto`. This was achieved by moving ports from `bolt402-core` to `bolt402-proto`.

3. **Real backends compiled to WASM** — `bolt402-lnd` (REST feature) and `bolt402-swissknife` both use `reqwest`, which compiles to `wasm32-unknown-unknown` using browser `fetch`. No JS callback delegation needed. Wrapped as `WasmLndRestBackend` and `WasmSwissKnifeBackend`.

4. **Conditional async_trait** — Port traits use `#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]` because `reqwest::Response` is not `Send` on WASM targets.

5. **Conditional platform APIs** — `danger_accept_invalid_certs()` and `from_env()` are gated behind `#[cfg(not(target_arch = "wasm32"))]` since they don't apply in browsers.

6. **In-process mock for testing** — `WasmMockServer` + `WasmMockClient` simulate the full L402 flow without any HTTP server. Uses `js_sys::Math::random()` for randomness and `js_sys::Date::now()` for timestamps on WASM targets.

7. **Budget in WASM** — Full budget enforcement (per-request, hourly, daily, total) using `js_sys::Date` for timestamps instead of `SystemTime` (which panics in WASM).

### API Surface

```typescript
// Real LND REST backend (makes actual HTTP calls via fetch)
const lnd = new WasmLndRestBackend("https://localhost:8080", "deadbeef...");
const info = await lnd.getInfo();
const payment = await lnd.payInvoice("lnbc...", 100);

// Real SwissKnife backend
const sk = new WasmSwissKnifeBackend("https://app.numeraire.tech", "sk-...");

// Mock server (in-process, no HTTP)
const server = new WasmMockServer({ "/api/data": 10n });
const client = new WasmMockClient(server, 100n);
const response = client.get("/api/data");
// response.status === 200, response.paid === true

// Budget
const budget = new WasmBudget(1000n, null, 50000n, null);
const budgetedClient = WasmMockClient.withBudget(server, 100n, budget);

// Utilities
const { macaroon, invoice } = parseL402Challenge(headerValue);
const header = buildL402Header(macaroon, preimage);
```

### Crate Structure

```
crates/bolt402-wasm/
├── Cargo.toml
├── src/
│   ├── lib.rs          # Mock server/client, utilities, wasm-bindgen exports
│   └── backends.rs     # WasmLndRestBackend, WasmSwissKnifeBackend wrappers
├── tests/
│   └── web.rs          # wasm-pack test (headless browser)
└── README.md
```

### Dependencies

- `bolt402-proto` — Types, port traits, errors (WASM-safe)
- `bolt402-lnd` (default-features = false, features = ["rest"]) — LND REST backend
- `bolt402-swissknife` — SwissKnife REST backend
- `wasm-bindgen` — Core WASM-JS bridge
- `wasm-bindgen-futures` — async/Promise interop
- `js-sys` — JS standard library access
- `serde-wasm-bindgen` — Serde-JsValue conversion

### Testing Plan

- `cargo test -p bolt402-wasm` — Native unit tests (mock challenge generation, budget, etc.)
- `wasm-pack test --headless --chrome crates/bolt402-wasm` — Browser tests via wasm-bindgen-test
- CI: `wasm-pack build` + both test suites

## CI

The `wasm` job builds and tests the WASM bindings. The `typescript` job depends on `wasm` and builds `bolt402-wasm` before `yarn install` (since `bolt402-ai-sdk` depends on `bolt402-wasm@file:../../crates/bolt402-wasm/pkg`).

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
      run: wasm-pack build crates/bolt402-wasm --target web
    - name: Run native unit tests
      run: cargo test -p bolt402-wasm
    - name: Run WASM browser tests
      run: wasm-pack test --headless --chrome crates/bolt402-wasm

typescript:
  name: TypeScript (bolt402-ai-sdk)
  needs: wasm
  runs-on: ubuntu-latest
  steps:
    # ... builds WASM first, then yarn install + tsc + vitest
```
