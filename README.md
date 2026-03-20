# bolt402

**L402 client SDK for AI agent frameworks**. Pay for APIs with Lightning.

bolt402 gives AI agents (LangChain, Vercel AI SDK, CrewAI, etc.) the ability to autonomously pay for L402-gated APIs using the Lightning Network. Built in Rust with a TypeScript package for the Vercel AI SDK.

## Why?

Lightning Labs' [agent-kit](https://github.com/lightninglabs/agent-kit) provides `lnget`, a CLI tool for L402 payments. Great for shell-based agents, but the AI agent ecosystem is library-based. LangChain has 200M+ monthly PyPI downloads. These frameworks need a native library, not a shell command.

bolt402 fills that gap.

## Architecture

```
┌─────────────────────────────────────┐
│        Agent Framework              │
│  (LangChain / Vercel AI / CrewAI)   │
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
│  ├── LND (gRPC)                     │
│  ├── SwissKnife (REST)              │
│  ├── Mock (testing)                 │
│  └── Custom (implement LnBackend)   │
└─────────────────────────────────────┘
```

Hexagonal (ports & adapters) architecture. Core logic has zero external dependencies. Lightning backends and token stores are traits with pluggable implementations.

See [docs/architecture.md](docs/architecture.md) for the full design breakdown.

## Packages

| Package | Description | Status |
|---------|-------------|--------|
| [`bolt402-proto`](crates/bolt402-proto) | L402 protocol types, challenge parsing, token construction | ✅ Complete |
| [`bolt402-core`](crates/bolt402-core) | Client engine, ports, budget tracker, token cache, receipts | ✅ Complete |
| [`bolt402-lnd`](crates/bolt402-lnd) | LND gRPC backend adapter | ✅ Complete |
| [`bolt402-swissknife`](crates/bolt402-swissknife) | SwissKnife REST backend adapter | ✅ Complete |
| [`bolt402-mock`](crates/bolt402-mock) | Mock L402 server for testing (no real Lightning needed) | ✅ Complete |
| [`bolt402-ai-sdk`](packages/bolt402-ai-sdk) | Vercel AI SDK tools (TypeScript) | ✅ Complete |
| [`bolt402-ffi`](crates/bolt402-ffi) | C-compatible FFI layer for Go/Swift/Kotlin bindings | ✅ Complete |
| [`bolt402-python`](crates/bolt402-python) | Python bindings via PyO3 | ✅ Complete |
| [`bolt402-go`](bindings/bolt402-go) | Go bindings via CGo | ✅ Complete |
| [`bolt402-wasm`](crates/bolt402-wasm) | WebAssembly bindings via wasm-pack | ✅ Complete |

## Quick Start (Rust)

```rust
use bolt402_core::{L402Client, L402ClientConfig};
use bolt402_core::budget::Budget;
use bolt402_core::cache::InMemoryTokenStore;
use bolt402_lnd::LndBackend;

#[tokio::main]
async fn main() {
    let backend = LndBackend::new(
        "https://localhost:10009",
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
import { createBolt402Tools, LndBackend } from 'bolt402-ai-sdk';
import { generateText } from 'ai';
import { openai } from '@ai-sdk/openai';

const tools = createBolt402Tools({
  backend: new LndBackend({
    url: 'https://localhost:8080',
    macaroon: process.env.LND_MACAROON!,
  }),
  budget: { perRequestMax: 1000, dailyMax: 50_000 },
});

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
|---------|-------------|-----|
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

## Roadmap

- [x] Core L402 client engine
- [x] LND gRPC backend
- [x] SwissKnife REST backend
- [x] Mock L402 server
- [x] CI/CD pipeline
- [x] Vercel AI SDK integration
- [x] Comprehensive documentation
- [ ] Python bindings (PyO3)
- [ ] TypeScript bindings (napi-rs)
- [ ] Go bindings (cgo)
- [ ] WASM bindings (wasm-pack)
- [ ] LangChain integration
- [ ] MCP server mode

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contributing

Contributions welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding standards, and PR workflow. Open an issue first to discuss before starting work.
