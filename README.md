<div align="center">
  <h1>bolt402</h1>

  <p>
    <strong>L402 client SDK for AI agent frameworks. Pay for APIs with Lightning.</strong>
  </p>

  <p>
    <a href="https://crates.io/crates/bolt402-core"><img alt="crates.io" src="https://img.shields.io/crates/v/bolt402-core.svg"/></a>
    <a href="https://www.npmjs.com/package/@lightninglabs/bolt402"><img alt="npm (WASM)" src="https://img.shields.io/npm/v/@lightninglabs/bolt402.svg?label=npm%20(wasm)"/></a>
    <a href="https://www.npmjs.com/package/@lightninglabs/bolt402-ai"><img alt="npm (AI SDK)" src="https://img.shields.io/npm/v/@lightninglabs/bolt402-ai.svg?label=npm%20(ai-sdk)"/></a>
    <a href="https://pypi.org/project/bolt402/"><img alt="PyPI" src="https://img.shields.io/pypi/v/bolt402.svg?label=pypi"/></a>
    <a href="https://github.com/lightninglabs/bolt402/blob/main/LICENSE-MIT"><img alt="MIT or Apache-2.0 Licensed" src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg"/></a>
    <a href="https://blog.rust-lang.org/2025/02/20/Rust-1.85.0.html"><img alt="Rustc Version 1.85.0+" src="https://img.shields.io/badge/rustc-1.85.0%2B-lightgrey.svg"/></a>
  </p>

</div>

bolt402 gives AI agents the ability to autonomously pay for [L402](https://docs.lightning.engineering/the-lightning-network/l402)-gated APIs using the Lightning Network. Built in Rust with TypeScript and Python bindings.

## Install

```bash
# Rust
cargo add bolt402-core bolt402-lnd

# TypeScript (Vercel AI SDK)
yarn add @lightninglabs/bolt402-ai

# TypeScript (WASM bindings only)
yarn add @lightninglabs/bolt402

# Python
pip install bolt402

# Python (LangChain tools)
pip install bolt402-langchain
```

## Quick Start

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

See the [bolt402-ai-sdk README](packages/bolt402-ai-sdk/README.md) for full TypeScript documentation and the [L402 Explorer](demos/l402-explorer) for an interactive demo.

## Packages

| Package | Description |
| ------- | ----------- |
| [`bolt402-core`](crates/bolt402-core) | L402 client engine, budget tracker, token cache, receipts |
| [`bolt402-proto`](crates/bolt402-proto) | Protocol types and port traits (`LnBackend`, `TokenStore`) |
| [`bolt402-lnd`](crates/bolt402-lnd) | LND backend (gRPC + REST) |
| [`bolt402-cln`](crates/bolt402-cln) | Core Lightning backend (gRPC + REST) |
| [`bolt402-nwc`](crates/bolt402-nwc) | Nostr Wallet Connect (NIP-47) backend |
| [`bolt402-swissknife`](crates/bolt402-swissknife) | Numeraire SwissKnife backend |
| [`bolt402-mock`](crates/bolt402-mock) | Mock L402 server for testing |
| [`bolt402-sqlite`](crates/bolt402-sqlite) | SQLite persistent token store |
| [`@lightninglabs/bolt402`](crates/bolt402-wasm) | WebAssembly bindings (npm) |
| [`@lightninglabs/bolt402-ai`](packages/bolt402-ai-sdk) | Vercel AI SDK tools (npm) |
| [`bolt402-ffi`](crates/bolt402-ffi) | C FFI layer for Go/Swift/Kotlin |
| [`bolt402-python`](crates/bolt402-python) | Python bindings (PyO3) |
| [`bolt402-go`](bindings/bolt402-go) | Go bindings (CGo) |
| [`bolt402-langchain`](packages/bolt402-langchain) | LangChain Python tools |

## Supported Lightning Backends

- **LND** — gRPC and REST
- **Core Lightning (CLN)** — gRPC and REST
- **Nostr Wallet Connect (NWC)** — NIP-47
- **Numeraire SwissKnife** — REST
- **Mock** — for testing without a real node
- **Custom** — implement the `LnBackend` trait

## Documentation

- [Architecture Guide](docs/architecture.md)
- [Getting Started](docs/tutorials/getting-started.md) — First L402 payment with mock server
- [Custom Backend](docs/tutorials/custom-backend.md) — Implement `LnBackend` for your node
- [Budget Control](docs/tutorials/budget-control.md) — Spending limits for autonomous agents
- [CONTRIBUTING.md](CONTRIBUTING.md) — Development setup and PR workflow
- [CHANGELOG.md](CHANGELOG.md)

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.

## Contributing

Contributions welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) first.
