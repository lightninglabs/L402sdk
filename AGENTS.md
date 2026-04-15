# AGENTS.md

## Overview

L402sdk is an L402 client SDK for AI agent frameworks. It enables programmatic Lightning payments for L402-gated APIs, built in Rust with FFI bindings for Python, Go, and WASM, plus native integrations for Vercel AI SDK and LangChain.

**Author:** Dario Anongba Varela <dario.anongba@gmail.com>
**Organization:** Lightning Labs

## Architecture

Hexagonal (ports & adapters) / Clean Architecture / Domain-Driven Design.

```
l402/
├── crates/
│   ├── l402-proto/       # L402 protocol types, challenge parsing, token construction
│   ├── l402-core/        # Client SDK core: L402 engine, cache, budget, receipts
│   │                        # Ports: LnBackend, TokenStore (trait definitions)
│   │                        # Adapters: InMemoryTokenStore, BudgetTracker
│   ├── l402-lnd/         # LND gRPC backend adapter
│   ├── l402-cln/         # CLN (Core Lightning) gRPC backend adapter
│   ├── l402-nwc/         # Nostr Wallet Connect (NIP-47) backend adapter
│   ├── l402-swissknife/  # SwissKnife REST backend adapter
│   ├── l402-mock/        # Mock L402 server for testing and development
│   ├── l402-sqlite/      # SQLite persistent token store
│   ├── l402-ffi/         # C-compatible FFI layer for Go/Swift/Kotlin bindings
│   ├── l402-python/      # Python bindings via PyO3/maturin
│   └── l402-wasm/        # WebAssembly bindings via wasm-pack
├── bindings/
│   └── l402-go/          # Go bindings via CGo + l402-ffi
├── packages/
│   ├── l402-ai-sdk/      # Vercel AI SDK tools (TypeScript)
│   └── l402-langchain/   # LangChain Python integration
├── demos/
│   ├── l402-explorer/       # Interactive L402 web app (Next.js)
│   └── comparison/          # L402sdk vs lnget comparison page
├── examples/
│   ├── basic-mock/          # Full L402 flow with mock server
│   ├── budget-control/      # Budget limits and rejection
│   ├── ai-agent/            # Vercel AI SDK + L402sdk
│   └── langchain/           # LangChain + L402sdk
├── docs/
│   ├── architecture.md      # Hexagonal design, crate graph, protocol flow
│   ├── design/              # Design documents (numbered)
│   └── tutorials/           # Getting started, custom backend, budget control
├── AGENTS.md                # This file
├── CLAUDE.md                # Instructions for Claude Code / Codex agents
├── PROJECT.md               # Project brief, initial request, status tracker
├── CONTRIBUTING.md          # Development setup, coding standards, PR workflow
├── CHANGELOG.md             # Release history
└── Cargo.toml               # Workspace manifest
```

### Crate Dependency Graph

```
l402-proto    (no internal deps, shared protocol types)
     ↑
l402-core     (depends on proto: client engine, ports, adapters)
     ↑
├── l402-lnd         (implements LnBackend for LND gRPC)
├── l402-cln         (implements LnBackend for CLN gRPC)
├── l402-nwc         (implements LnBackend for Nostr Wallet Connect)
├── l402-swissknife  (implements LnBackend for SwissKnife REST)
├── l402-sqlite      (implements TokenStore with SQLite)
├── l402-ffi         (C FFI layer exposing core API)
├── l402-python      (PyO3 bindings)
└── l402-wasm        (wasm-pack bindings)

l402-mock     (depends on proto: standalone mock L402 server)
```

### Design Principles

1. **Ports & Adapters**: Core business logic has zero external dependencies. Lightning backends and token stores are traits (ports). Implementations are adapters in separate crates.
2. **Domain types in proto**: `L402Challenge`, `L402Token`, `L402Error` live in `l402-proto` so both client and server implementations can share them.
3. **Error handling**: `thiserror` for typed errors, no `anyhow` in library code (only in binaries/tests).
4. **Async-first**: All port traits are `async_trait` (with `?Send` on WASM). `l402-core` has no async runtime dependency — uses `std::sync::RwLock` and `web_time::Instant`. Only `l402-lnd[grpc]` requires tokio (via tonic).

## Development Workflow

### Commits

- **Conventional commits**: `feat:`, `fix:`, `chore:`, `test:`, `docs:`, `ci:`, `refactor:`
- One logical change per commit
- PRs are squash-merged, one clean commit per PR

### PRs

- Every change goes through a PR, even for the maintainer
- PR description includes: what changed, why, how to test
- CI must be green before merge
- Squash merge only

### Testing

- Unit tests in each module (`#[cfg(test)] mod tests`)
- Integration tests in `tests/` directory
- `l402-mock` provides a mock L402 server for integration testing
- Python: pytest for l402-langchain tests
- TypeScript: vitest for l402-ai-sdk tests
- CI runs: `cargo fmt --check`, `cargo clippy`, `cargo test`, `cargo doc`, FFI build, WASM build, Python tests, TypeScript tests, LangChain tests

### Code Style

- `rustfmt` with project config (see `rustfmt.toml`)
- `clippy` with project config (see `clippy.toml`)
- Documentation on all public items (enforced by `#![warn(missing_docs)]`)
- Examples in doc comments where useful

## For AI Agents (Claude Code, Codex, etc.)

See `CLAUDE.md` for specific instructions when working on this codebase.

## Key Decisions

- **Rust-first**: Core protocol engine in Rust for correctness and multi-language FFI
- **L402sdk name**: Renamed from `lnpay` (already taken). "bolt" references BOLT specs, "402" references HTTP 402
- **Dual license**: MIT OR Apache-2.0 (matches Rust ecosystem convention)
- **Edition 2024**: Using latest Rust edition
- **MSRV 1.85**: Minimum supported Rust version
- **No autonomous merges in cron jobs**: Open PRs, leave for Dario's review
- **No standalone binaries**: L402sdk is a library/SDK. MCP server, CLI tools belong elsewhere

## References

- [L402 Protocol](https://docs.lightning.engineering/the-lightning-network/l402)
- [Original proposal](../lnpay-proposal.md)
- [Lightning Labs lightning-agent-tools](https://github.com/lightninglabs/lightning-agent-tools)
- [Aperture (L402 reverse proxy)](https://github.com/lightninglabs/aperture)
- [Numeraire Swissknife](https://github.com/bitcoin-numeraire/swissknife) — reference for CLN REST/LND REST adapters
