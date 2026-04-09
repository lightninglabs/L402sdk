<div align="center">
  <h1>bolt402-wasm</h1>

  <p>
    <strong>WebAssembly bindings for the bolt402 L402 client SDK</strong>
  </p>

  <p>
    <a href="https://www.npmjs.com/package/@lightninglabs/bolt402"><img alt="npm" src="https://img.shields.io/npm/v/@lightninglabs/bolt402.svg"/></a>
    <a href="https://www.npmjs.com/package/@lightninglabs/bolt402"><img alt="npm downloads" src="https://img.shields.io/npm/dm/@lightninglabs/bolt402.svg"/></a>
    <a href="https://github.com/lightninglabs/bolt402/blob/main/LICENSE-MIT"><img alt="MIT or Apache-2.0 Licensed" src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg"/></a>
  </p>

</div>

Run L402 Lightning payment flows in browsers, Deno, Cloudflare Workers, and any WASM runtime.

## Install

```bash
yarn add @lightninglabs/bolt402
```

## Overview

bolt402-wasm compiles the Rust L402 protocol engine to WebAssembly via `wasm-pack`, providing:

- **`WasmL402Client`** — the real Rust `L402Client`, compiled to WASM
- **Direct backend wrappers** — `WasmLndRestBackend`, `WasmClnRestBackend`, and `WasmSwissKnifeBackend`
- **Budget configuration and receipts** — shared Rust behavior for spending limits and audit data
- **Auto-generated TypeScript types** — full type safety in TS/JS projects

## Quick Start

```javascript
import init, { WasmL402Client, WasmBudgetConfig } from 'bolt402-wasm';

// Initialize the WASM module
await init();

// Create a client backed by LND REST
const client = WasmL402Client.withLndRest(
  'https://localhost:8080',
  'hex-encoded-admin-macaroon',
  WasmBudgetConfig.unlimited(),
  100,
);

// Make a request — L402 payment happens automatically
const response = await client.get('https://api.example.com/paid-resource');
console.log(response.status);   // 200
console.log(response.paid);     // true or false if cached
console.log(response.body);

// Inspect the payment receipt
const receipt = response.receipt;
console.log(receipt.amountSats);
console.log(receipt.paymentHash);   // hex string
console.log(receipt.preimage);      // hex string

// Token caching: second request uses cached token (no payment)
const cached = await client.get('https://api.example.com/paid-resource');
console.log(cached.paid);  // false (used cached token)

// Track spending
console.log(await client.totalSpent());
console.log(await client.receipts());
```

## Budget Enforcement

```javascript
const budget = new WasmBudgetConfig(
  100,     // per-request max: 100 sats
  1000,    // hourly max: 1,000 sats
  5000,    // daily max: 5,000 sats
  50000,   // total max: 50,000 sats
);

const client = WasmL402Client.withLndRest(
  'https://localhost:8080',
  'hex-encoded-admin-macaroon',
  budget,
  100,
);

// Requests exceeding the budget will reject
try {
  await client.get('https://api.example.com/expensive');
} catch (e) {
  console.error(e); // "payment of X sats exceeds per-request limit"
}
```

## Direct Backend Wrappers

Use the backend wrappers when you want Lightning-node access from JS/TS without going through the full `WasmL402Client` flow.

### LND REST

```javascript
import init, { WasmLndRestBackend } from 'bolt402-wasm';

await init();

const lnd = new WasmLndRestBackend('https://localhost:8080', 'deadbeef...');
const info = await lnd.getInfo();
console.log(info.alias);
```

### CLN REST

```javascript
import init, { WasmClnRestBackend } from 'bolt402-wasm';

await init();

const cln = WasmClnRestBackend.withRune(
  'https://localhost:3010',
  'rune-token-value...',
);
const info = await cln.getInfo();
console.log(info.alias);
```

### SwissKnife

```javascript
import init, { WasmSwissKnifeBackend } from 'bolt402-wasm';

await init();

const backend = new WasmSwissKnifeBackend(
  'https://api.numeraire.tech',
  'sk-...',
);
const balance = await backend.getBalance();
console.log(balance);
```

## Building

```bash
# Prerequisites
rustup target add wasm32-unknown-unknown
cargo install wasm-pack

# Build for web
wasm-pack build crates/bolt402-wasm --target web

# Build for Node.js
wasm-pack build crates/bolt402-wasm --target nodejs

# Build for bundlers (webpack, etc.)
wasm-pack build crates/bolt402-wasm --target bundler
```

## Testing

```bash
# Native unit tests
cargo test -p bolt402-wasm

# WASM browser tests (requires Chrome/Firefox)
wasm-pack test --headless --chrome crates/bolt402-wasm
```

## Architecture

bolt402-wasm provides the real Rust L402 client and backend adapters over `wasm-bindgen`:

```
┌─────────────────────────────────┐
│        JavaScript / TS          │
│     (browser / Node / Deno)     │
└─────────────┬───────────────────┘
              │ wasm-bindgen
┌─────────────▼───────────────────┐
│         bolt402-wasm            │
│                                 │
│  WasmL402Client                 │
│  WasmLndRestBackend             │
│  WasmClnRestBackend             │
│  WasmSwissKnifeBackend          │
└─────────────────────────────────┘
              │
   ┌──────────┼───────────┐
   ▼          ▼           ▼
 bolt402-  bolt402-   bolt402-
  core       lnd         cln
              │           │
              ▼           ▼
        LND REST API   CLN REST API
```

## License

MIT OR Apache-2.0
