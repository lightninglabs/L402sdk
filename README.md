<div align="center">
  <h1>bolt402</h1>

  <p>
    <strong>L402 client SDK for AI agent frameworks. Pay for APIs with Lightning.</strong>
  </p>

  <p>
    <a href="https://crates.io/crates/bolt402-core"><img alt="crates.io" src="https://img.shields.io/crates/v/bolt402-core.svg"/></a>
    <a href="https://www.npmjs.com/package/@lightninglabs/bolt402"><img alt="npm (WASM)" src="https://img.shields.io/npm/v/@lightninglabs/bolt402.svg?label=npm%20(wasm)"/></a>
    <a href="https://www.npmjs.com/package/@lightninglabs/bolt402-ai"><img alt="npm (AI SDK)" src="https://img.shields.io/npm/v/@lightninglabs/bolt402-ai.svg?label=npm%20(ai-sdk)"/></a>
    <a href="https://github.com/lightninglabs/bolt402/blob/main/LICENSE-MIT"><img alt="MIT or Apache-2.0 Licensed" src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg"/></a>
    <a href="https://blog.rust-lang.org/2025/02/20/Rust-1.85.0.html"><img alt="Rustc Version 1.85.0+" src="https://img.shields.io/badge/rustc-1.85.0%2B-lightgrey.svg"/></a>
  </p>

</div>

bolt402 gives AI agents the ability to autonomously pay for L402-gated APIs using the Lightning Network. Built in Rust with TypeScript and Python bindings.

## Install

```bash
# Rust
cargo add bolt402-core bolt402-lnd

# TypeScript (WASM bindings)
yarn add @lightninglabs/bolt402

# TypeScript (Vercel AI SDK tools)
yarn add @lightninglabs/bolt402-ai

# Python
pip install bolt402
```

## Why?

Lightning Labs' [lightning-agent-tools](https://github.com/lightninglabs/lightning-agent-tools) provides `lnget`, a CLI tool for L402 payments. Great for shell-based agents, but the AI agent ecosystem is library-based. LangChain has 200M+ monthly PyPI downloads. These frameworks need a native library, not a shell command.

bolt402 fills that gap.

## Architecture

```
┌─────────────────────────────────────┐
│        Agent Framework              │
│  (Vercel AI SDK / LangChain / etc.)  │
└───────────────┬─────────────────────┘
                │
┌───────────────▼─────────────────────┐
│           bolt402 SDK               │
│                                     │
│  L402 Engine ─── Token Cache        │
│       │                             │
│  Budget Tracker ── Receipt Logger   │
│       │                             │
│  Lightning Backend (pluggable)      │
│  ├── LND (gRPC + REST)              │
│  ├── CLN (gRPC + REST)              │
│  ├── NWC (Nostr Wallet Connect)     │
│  ├── SwissKnife (REST)              │
│  ├── Mock (testing)                 │
│  └── Custom (implement LnBackend)   │
└─────────────────────────────────────┘
```

Hexagonal (ports & adapters) architecture. Core logic has zero external dependencies. Lightning backends and token stores are traits with pluggable implementations.

See [docs/architecture.md](docs/architecture.md) for the full design breakdown.

## Packages

| Package | Description |  
| ------------ | ------------- |
| [`bolt402-proto`](crates/bolt402-proto) | L402 protocol types, port traits (`LnBackend`, `TokenStore`), `ClientError`. WASM-safe, no async runtime dependency. |
| [`bolt402-core`](crates/bolt402-core) | L402 client engine (`L402Client`), budget tracker, in-memory token cache, receipts. No async runtime dependency (WASM-compatible). |
| [`bolt402-lnd`](crates/bolt402-lnd) | LND backend: gRPC (feature `grpc`) + REST (feature `rest`, WASM-compatible) |
| [`bolt402-cln`](crates/bolt402-cln) | Core Lightning (CLN) backends: gRPC (feature `grpc`) + REST (feature `rest`, WASM-compatible) |
| [`bolt402-nwc`](crates/bolt402-nwc) | Nostr Wallet Connect (NIP-47) backend adapter |
| [`bolt402-swissknife`](crates/bolt402-swissknife) | SwissKnife REST backend adapter (WASM-compatible) |
| [`bolt402-mock`](crates/bolt402-mock) | Mock L402 server for testing (no real Lightning needed) |
| [`bolt402-sqlite`](crates/bolt402-sqlite) | SQLite persistent token store (survives restarts) |
| [`bolt402-wasm`](crates/bolt402-wasm) | WebAssembly bindings: Rust L402 client plus direct LND REST, CLN REST, and SwissKnife backend wrappers |
| [`bolt402-ai-sdk`](packages/bolt402-ai-sdk) | Vercel AI SDK tools (TypeScript). Thin wrapper around bolt402-wasm — all L402 logic in Rust/WASM | 
| [`bolt402-ffi`](crates/bolt402-ffi) | C-compatible FFI layer for Go/Swift/Kotlin bindings |
| [`bolt402-python`](crates/bolt402-python) | Python bindings via PyO3 |
| [`bolt402-go`](bindings/bolt402-go) | Go bindings via CGo |
| [`bolt402-langchain`](packages/bolt402-langchain) | LangChain Python tools (L402FetchTool, BudgetTool, callbacks) |

## Quick Start (Rust)

```rust
use bolt402_core::L402Client;
use bolt402_core::budget::Budget;
use bolt402_core::cache::InMemoryTokenStore;
use bolt402_lnd::LndGrpcBackend;

#[tokio::main]
async fn main() {
    let backend = LndGrpcBackend::connect(
        "https://localhost:10009",
        "/path/to/tls.cert",
        "/path/to/admin.macaroon",
    ).await.unwrap();

    let client = L402Client::builder()
        .ln_backend(backend)
        .token_store(InMemoryTokenStore::default())
        .budget(Budget {
            per_request_max: Some(1_000),
            daily_max: Some(50_000),
            ..Budget::unlimited()
        })
        .build()
        .unwrap();

    // L402 negotiation happens automatically
    let response = client.get("https://api.example.com/paid-resource").await.unwrap();
    println!("Status: {}", response.status());

    if response.paid() {
        let receipt = response.receipt().unwrap();
        println!("Paid {} sats", receipt.total_cost_sats());
    }
}
```

## Quick Start (Vercel AI SDK)

```typescript
import { createBolt402Tools, WasmL402Client, WasmBudgetConfig } from '@lightninglabs/bolt402-ai';
import { generateText } from 'ai';
import { openai } from '@ai-sdk/openai';

const client = WasmL402Client.withLndRest(
  'https://localhost:8080',
  process.env.LND_MACAROON!,
  new WasmBudgetConfig(1000, 0, 50_000, 0),
  100,
);

const tools = createBolt402Tools({ client });

const result = await generateText({
  model: openai('gpt-4o'),
  tools,
  maxSteps: 5,
  prompt: 'Fetch the premium data from https://api.example.com/v1/data',
});
```

See the [bolt402-ai-sdk README](packages/bolt402-ai-sdk/README.md) for full TypeScript documentation.

## Try Without a Lightning Node

Use `bolt402-mock` to test the full L402 flow without any real infrastructure:

```rust
use bolt402_mock::{MockL402Server, EndpointConfig};

let server = MockL402Server::builder()
    .endpoint("/api/data", EndpointConfig::new(100))
    .build()
    .await
    .unwrap();

let client = L402Client::builder()
    .ln_backend(server.mock_backend())
    .token_store(InMemoryTokenStore::default())
    .budget(Budget::unlimited())
    .build()
    .unwrap();

let response = client.get(&format!("{}/api/data", server.url())).await.unwrap();
assert!(response.paid());
```

See the [Getting Started tutorial](docs/tutorials/getting-started.md) for a full walkthrough.

## Examples

| Example | Description | Run |
| --------- | ------------- |----- |
| [basic-mock](examples/basic-mock) | Full L402 flow with mock server | `cargo run -p example-basic-mock` |
| [budget-control](examples/budget-control) | Budget limits and rejection | `cargo run -p example-budget-control` |
| [mock demo](crates/bolt402-mock/examples/demo.rs) | Interactive demo (in bolt402-mock) | `cargo run -p bolt402-mock --example demo` |
| [ai-agent](examples/ai-agent) | Vercel AI SDK + bolt402 | `cd examples/ai-agent && npx tsx index.ts` |

## Documentation

- [Architecture Guide](docs/architecture.md) — Hexagonal design, crate graph, protocol flow
- **Tutorials:**
  - [Getting Started](docs/tutorials/getting-started.md) — First L402 payment with mock server
  - [Custom Backend](docs/tutorials/custom-backend.md) — Implement LnBackend for your Lightning node
  - [Budget Control](docs/tutorials/budget-control.md) — Spending limits for autonomous agents
- [CONTRIBUTING.md](CONTRIBUTING.md) — Development setup, coding standards, PR workflow
- [CHANGELOG.md](CHANGELOG.md) — Release history

## Development

```bash
cargo build          # Build all crates
cargo test           # Run all tests
cargo fmt --check    # Check formatting
cargo clippy         # Lint
cargo doc --no-deps  # Build docs
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contributing

Contributions welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding standards, and PR workflow. Open an issue first to discuss before starting work.
