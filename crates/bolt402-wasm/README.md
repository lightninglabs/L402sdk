# bolt402-wasm

WebAssembly bindings for the [bolt402](https://github.com/bitcoin-numeraire/bolt402) L402 client SDK. Run L402 Lightning payment flows in browsers, Deno, Cloudflare Workers, and any WASM runtime.

## Overview

bolt402-wasm compiles the Rust L402 protocol engine to WebAssembly via `wasm-pack`, providing:

- **In-process mock L402 server** — test L402 flows without any HTTP server or Lightning node
- **Full L402 protocol flow** — challenge parsing, payment simulation, token caching, budget enforcement
- **Payment receipts** — structured proof-of-payment data for audit and cost tracking
- **Auto-generated TypeScript types** — full type safety in TS/JS projects

## Quick Start

```javascript
import init, { WasmMockServer, WasmMockClient, WasmBudget } from 'bolt402-wasm';

// Initialize the WASM module
await init();

// Create a mock server with priced endpoints
const server = new WasmMockServer({
  "/api/data": 10,      // 10 sats
  "/api/premium": 100,  // 100 sats
});

// Create a client connected to the mock server
const client = new WasmMockClient(server, 100n); // max 100 sat fee

// Make a request — L402 payment happens automatically
const response = client.get("/api/data");
console.log(response.status);   // 200
console.log(response.paid);     // true
console.log(response.body);     // '{"ok":true,"price":10}'

// Inspect the payment receipt
const receipt = response.receipt;
console.log(receipt.amountSats);    // 10n
console.log(receipt.paymentHash);   // hex string
console.log(receipt.preimage);      // hex string

// Token caching: second request uses cached token (no payment)
const cached = client.get("/api/data");
console.log(cached.paid);  // false (used cached token)

// Track spending
console.log(client.totalSpent);    // 10n
console.log(client.paymentCount);  // 1
```

## Budget Enforcement

```javascript
const budget = new WasmBudget(
  100n,    // per-request max: 100 sats
  1000n,   // hourly max: 1,000 sats
  5000n,   // daily max: 5,000 sats
  50000n,  // total max: 50,000 sats
);

const client = WasmMockClient.withBudget(server, 100n, budget);

// Requests exceeding the budget will throw an error
try {
  client.get("/api/expensive"); // > 100 sats
} catch (e) {
  console.error(e); // "payment of X sats exceeds per-request limit"
}
```

## Utility Functions

```javascript
import { parseL402Challenge, buildL402Header, version } from 'bolt402-wasm';

// Parse a WWW-Authenticate header
const challenge = parseL402Challenge(
  'L402 macaroon="YWJjZGVm", invoice="lnbc100n1..."'
);
console.log(challenge.macaroon); // "YWJjZGVm"
console.log(challenge.invoice);  // "lnbc100n1..."

// Build an Authorization header
const header = buildL402Header("YWJjZGVm", "abcdef1234567890");
// "L402 YWJjZGVm:abcdef1234567890"

// Check version
console.log(version()); // "0.1.0"
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

bolt402-wasm provides an **in-process** mock environment — no HTTP server, no WebSocket, no real Lightning node. The mock server and client communicate directly in memory, simulating the full L402 protocol:

```
┌─────────────────────────────────┐
│        JavaScript / TS          │
│     (browser / Node / Deno)     │
└─────────────┬───────────────────┘
              │ wasm-bindgen
┌─────────────▼───────────────────┐
│         bolt402-wasm            │
│                                 │
│  WasmMockServer  WasmMockClient │
│      │                │         │
│      │   in-process   │         │
│      ├────────────────┤         │
│      │  L402 protocol │         │
│      │  challenge →   │         │
│      │  ← payment     │         │
│      │  → retry       │         │
│      │  ← 200 OK      │         │
│      └────────────────┘         │
│                                 │
│  Budget tracker, token cache,   │
│  receipt logger                 │
└─────────────────────────────────┘
```

## License

MIT OR Apache-2.0
