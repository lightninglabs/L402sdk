# Design Doc 045: WASM Bindings via wasm-pack

**Issue:** #45
**Date:** 2026-03-20
**Author:** Toshi (maintainer)
**Status:** Implementing

## Problem

bolt402 has native bindings for Python (PyO3), Go (CGo/FFI), and TypeScript (pure). However, the TypeScript package (`bolt402-ai-sdk`) is a pure TS implementation that duplicates protocol logic. Browser-based and edge-runtime AI agents cannot use the Rust core directly.

WASM bindings complete the cross-language story by enabling:

- **Browser AI agents** — L402 payments directly from the browser
- **Edge runtimes** — Cloudflare Workers, Deno Deploy, Vercel Edge Functions
- **Universal WASM runtimes** — Wasmtime, Wasmer, etc.

## Proposed Design

### New Crate: `crates/bolt402-wasm/`

A thin wasm-bindgen wrapper around `bolt402-core` and `bolt402-mock`, built with `wasm-pack`.

### Architecture

```
                  ┌─────────────────────┐
                  │   JavaScript/TS     │
                  │   (browser / Node)  │
                  └─────────┬───────────┘
                            │ wasm-bindgen
                  ┌─────────▼───────────┐
                  │    bolt402-wasm     │
                  │  (wasm-bindgen      │
                  │   wrapper types)    │
                  └─────────┬───────────┘
                            │
              ┌─────────────┼─────────────┐
              │             │             │
    ┌─────────▼──┐  ┌──────▼──────┐  ┌──▼──────────┐
    │bolt402-core│  │bolt402-mock │  │bolt402-proto │
    └────────────┘  └─────────────┘  └──────────────┘
```

### Key Decisions

1. **wasm-bindgen + wasm-pack** — Standard toolchain. Auto-generates TypeScript type definitions. npm-publishable.

2. **No tokio in WASM** — WASM doesn't support multi-threaded tokio. The mock server uses `axum` on tokio, which can't run in-browser. Strategy: expose a **mock client + in-process mock backend** that bypasses HTTP entirely, similar to the FFI approach.

3. **HTTP via fetch** — For real L402 requests, use `reqwest` with `wasm` feature (uses browser `fetch` under the hood). For mock testing, bypass HTTP entirely.

4. **Sync wrappers for mock, async for real** — Mock client operations are synchronous (in-process). Real L402Client operations return `Promise` via `wasm-bindgen-futures`.

5. **Budget in WASM** — Full `BudgetTracker` support using `js_sys::Date` for timestamps instead of `SystemTime` (which panics in WASM).

### API Surface

```typescript
// Mock server (in-process, no HTTP)
const server = new WasmMockServer({ "/api/data": 10n });
const client = new WasmMockClient(server, 100n);
const response = client.get("/api/data");
// response.status === 200, response.paid === true

// Budget
const budget = new WasmBudget({
  perRequestMax: 1000n,
  dailyMax: 50000n,
});

// Receipt inspection
const receipts = client.receipts(); // WasmReceipt[]
const totalSpent = client.totalSpent(); // bigint
```

### Crate Structure

```
crates/bolt402-wasm/
├── Cargo.toml
├── src/
│   └── lib.rs        # wasm-bindgen exports
├── tests/
│   └── web.rs        # wasm-pack test (headless browser)
└── README.md
```

### Dependencies

- `wasm-bindgen` — Core WASM↔JS bridge
- `wasm-bindgen-futures` — async/Promise interop
- `js-sys` — JS standard library access
- `serde-wasm-bindgen` — Serde↔JsValue conversion
- `bolt402-proto`, `bolt402-core`, `bolt402-mock` — internal crates

### Testing Plan

- `wasm-pack test --headless --chrome` (or `--node`)
- Test mock server creation, client GET/POST, budget enforcement, receipt tracking
- CI job: install wasm-pack, run wasm-pack build + test

### Alternatives Considered

1. **wasm-bindgen with full reqwest** — reqwest supports `wasm` target via fetch, but the mock server (axum) can't run in WASM. So we split: mock = in-process, real = fetch-based.

2. **Pure WASM without wasm-bindgen** — Possible but loses TypeScript type generation and ergonomic JS interop. Not worth the trade-off.

3. **Shared worker for mock server** — Too complex for testing purposes. In-process mock is simpler and sufficient.

## CI

New job in `.github/workflows/ci.yml`:

```yaml
wasm:
  name: WASM Bindings
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
      with:
        targets: wasm32-unknown-unknown
    - uses: cargo-bins/cargo-binstall@main
    - run: cargo binstall wasm-pack -y
    - run: wasm-pack build crates/bolt402-wasm --target web
    - run: wasm-pack test --headless --chrome crates/bolt402-wasm
```
