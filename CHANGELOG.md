# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **l402-core**: Removed tokio runtime dependency. Replaced `tokio::sync::RwLock` with `std::sync::RwLock` and `std::time::Instant` with `web_time::Instant`. The core library now compiles to WASM without an async runtime. (#63)
- **l402-ai-sdk**: Stripped all pure-TypeScript L402 reimplementation (l402-client, budget, token stores, backends, types). Now a thin wrapper around `WasmL402Client` from `l402-wasm`. (#63)
- **l402-regtest**: Expanded the Docker/Aperture integration suite to cover CLN REST in addition to LND gRPC/REST and CLN gRPC.

### Added

- **l402-lnd**: LND REST backend adapter (`LndRestBackend`) behind `rest` feature flag. WASM-compatible. (#63)
- **l402-cln**: Core Lightning (CLN) gRPC backend adapter implementing `LnBackend`. (#53)
- **l402-cln**: Core Lightning (CLN) REST backend adapter (`ClnRestBackend`) behind `rest` feature flag. Supports rune authentication and is WASM-compatible.
- **l402-nwc**: Nostr Wallet Connect (NIP-47) backend adapter implementing `LnBackend`. (#51)
- **l402-sqlite**: SQLite persistent token store implementing `TokenStore`. (#48)
- **l402-ffi**: C-compatible FFI layer for cross-language bindings. (#44)
- **l402-python**: Python bindings via PyO3/maturin. (#23)
- **l402-go** (bindings): Go bindings via CGo + l402-ffi. (#44)
- **l402-langchain**: LangChain Python integration with L402FetchTool, L402BudgetTool, PaymentCallbackHandler. (#57)
- BOLT11 invoice amount decoding for budget enforcement. (#21)
- L402 Explorer interactive demo (Next.js). (#34)
- AI Research Agent demo. (#36)
- L402sdk vs lnget comparison page. (#37)
- 402index.io MCP server integration for dynamic service discovery. (#41)
- CONTRIBUTING.md with development setup, coding standards, and PR workflow.
- GitHub issue templates for bug reports and feature requests.
- This CHANGELOG.md file.

### Removed

- **l402-ai-sdk**: Removed pure-TypeScript L402 client, budget tracker, token stores (InMemory, File, LocalStorage), and backend implementations. All protocol logic now runs in Rust via WASM. (#63)

## [0.1.0] — 2026-03-16

Initial development release. Not yet published to crates.io or npm.

### Added

- **l402-proto**: L402 protocol types, challenge parsing from `WWW-Authenticate` headers, token construction for `Authorization` headers, typed error hierarchy.
- **l402-core**: `L402Client` engine with automatic L402 negotiation (challenge → pay → retry), `InMemoryTokenStore` adapter, `BudgetTracker` with per-request and total spending limits, receipt logging. Port traits (`LnBackend`, `TokenStore`) live in `l402-proto`.
- **l402-lnd**: LND gRPC backend adapter implementing `LnBackend` via `SendPaymentV2` (router service). Vendored proto files for self-contained builds.
- **l402-swissknife**: SwissKnife REST API backend adapter implementing `LnBackend`.
- **l402-mock**: Mock L402 server (Axum-based) for integration testing and development. Configurable challenges, payment simulation, token validation.
- **l402-ai-sdk** (TypeScript): Vercel AI SDK integration providing `createL402Tools()` for AI agents to make L402-authenticated HTTP requests. LND and SwissKnife backend support.
- CI/CD pipeline: GitHub Actions for formatting, clippy, tests, documentation, and TypeScript checks.
- Makefile with `check`, `ci`, `build`, `test`, `lint`, `fmt`, `doc` targets.
- Comprehensive design documents for each feature (`docs/design/001` through `006`).
- Dual license: MIT OR Apache-2.0.

[Unreleased]: https://github.com/lightninglabs/L402sdk/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/lightninglabs/L402sdk/releases/tag/v0.1.0
