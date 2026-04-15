<div align="center">
  <h1>L402sdk</h1>

  <p>
    <strong>L402 client SDK for AI agent frameworks. Pay for APIs with Lightning.</strong>
  </p>

  <p>
    <a href="https://crates.io/crates/l402-core"><img alt="crates.io" src="https://img.shields.io/crates/v/l402-core.svg"/></a>
    <a href="https://www.npmjs.com/package/@lightninglabs/l402"><img alt="npm (WASM)" src="https://img.shields.io/npm/v/@lightninglabs/l402.svg?label=npm%20(wasm)"/></a>
    <a href="https://www.npmjs.com/package/@lightninglabs/l402-ai"><img alt="npm (AI SDK)" src="https://img.shields.io/npm/v/@lightninglabs/l402-ai.svg?label=npm%20(ai-sdk)"/></a>
    <a href="https://pypi.org/project/l402/"><img alt="PyPI" src="https://img.shields.io/pypi/v/L402sdk.svg?label=pypi"/></a>
    <a href="https://github.com/lightninglabs/L402sdk/blob/main/LICENSE-MIT"><img alt="MIT or Apache-2.0 Licensed" src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg"/></a>
    <a href="https://blog.rust-lang.org/2025/02/20/Rust-1.85.0.html"><img alt="Rustc Version 1.85.0+" src="https://img.shields.io/badge/rustc-1.85.0%2B-lightgrey.svg"/></a>
  </p>

</div>

L402sdk gives AI agents the ability to autonomously pay for [L402](https://docs.lightning.engineering/the-lightning-network/l402)-gated APIs using the Lightning Network. Built in Rust with TypeScript and Python bindings.

## Install

```bash
# Rust
cargo add l402-core l402-lnd

# TypeScript (Vercel AI SDK)
yarn add @lightninglabs/l402-ai

# TypeScript (WASM bindings only)
yarn add @lightninglabs/l402

# Python
pip install l402

# Python (LangChain tools)
pip install l402-langchain
```

## Quick Start

```typescript
import { createL402Tools, WasmL402Client, WasmBudgetConfig } from '@lightninglabs/l402-ai';
import { generateText } from 'ai';
import { openai } from '@ai-sdk/openai';

const client = WasmL402Client.withLndRest(
  'https://localhost:8080',
  process.env.LND_MACAROON!,
  new WasmBudgetConfig(1000, 0, 50_000, 0),
  100,
);

const tools = createL402Tools({ client });

const result = await generateText({
  model: openai('gpt-4o'),
  tools,
  maxSteps: 5,
  prompt: 'Fetch the premium data from https://api.example.com/v1/data',
});
```

See the [l402-ai-sdk README](packages/l402-ai-sdk/README.md) for full TypeScript documentation and the [L402 Explorer](demos/l402-explorer) for an interactive demo.

## Packages

| Package | Description |
| ------- | ----------- |
| [`l402-core`](crates/l402-core) | L402 client engine, budget tracker, token cache, receipts |
| [`l402-proto`](crates/l402-proto) | Protocol types and port traits (`LnBackend`, `TokenStore`) |
| [`l402-lnd`](crates/l402-lnd) | LND backend (gRPC + REST) |
| [`l402-cln`](crates/l402-cln) | Core Lightning backend (gRPC + REST) |
| [`l402-nwc`](crates/l402-nwc) | Nostr Wallet Connect (NIP-47) backend |
| [`l402-swissknife`](crates/l402-swissknife) | Numeraire SwissKnife backend |
| [`l402-mock`](crates/l402-mock) | Mock L402 server for testing |
| [`l402-sqlite`](crates/l402-sqlite) | SQLite persistent token store |
| [`@lightninglabs/l402`](crates/l402-wasm) | WebAssembly bindings (npm) |
| [`@lightninglabs/l402-ai`](packages/l402-ai-sdk) | Vercel AI SDK tools (npm) |
| [`l402-ffi`](crates/l402-ffi) | C FFI layer for Go/Swift/Kotlin |
| [`l402-python`](crates/l402-python) | Python bindings (PyO3) |
| [`l402-go`](bindings/l402-go) | Go bindings (CGo) |
| [`l402-langchain`](packages/l402-langchain) | LangChain Python tools |

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
