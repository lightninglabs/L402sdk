# AGENTS.md

## Overview

bolt402 is an L402 client SDK for AI agent frameworks. It enables programmatic Lightning payments for L402-gated APIs, built in Rust with planned FFI bindings for Python, TypeScript, Go, and WASM.

**Maintainer:** Toshi (AI agent, via OpenClaw)
**Owner:** Dario Anongba Varela <dario.anongba@gmail.com>
**Organization:** bitcoin-numeraire

## Architecture

Hexagonal (ports & adapters) / Clean Architecture / Domain-Driven Design.

```
bolt402/
├── crates/
│   ├── bolt402-proto/     # L402 protocol types, challenge parsing, token construction
│   ├── bolt402-core/      # Client SDK core: L402 engine, cache, budget, receipts
│   │                      # Ports: LnBackend, TokenStore (trait definitions)
│   │                      # Adapters: InMemoryTokenStore, BudgetTracker
│   ├── bolt402-lnd/       # LND gRPC backend adapter
│   ├── bolt402-mock/      # Mock L402 server for testing and development
│   ├── bolt402-swissknife/# SwissKnife REST API backend adapter
│   ├── bolt402-ffi/       # C-compatible FFI layer (cdylib/staticlib)
│   └── bolt402-python/    # Python bindings via PyO3
├── bindings/
│   └── bolt402-go/        # Go bindings via CGo
├── packages/
│   └── bolt402-ai-sdk/    # TypeScript/Vercel AI SDK integration
├── AGENTS.md              # This file
├── CLAUDE.md              # Instructions for Claude Code / Codex agents
├── PROJECT.md             # Project brief, initial request, status tracker
├── Cargo.toml             # Workspace manifest
└── ...
```

### Crate Dependency Graph

```
bolt402-proto  (no internal deps, shared protocol types)
     ↑
bolt402-core   (depends on proto: client engine, ports, adapters)
     ↑
bolt402-lnd    (depends on core: implements LnBackend for LND)

bolt402-mock   (depends on proto: standalone mock L402 server)

bolt402-ffi    (depends on core + mock: C ABI for FFI bindings)
     ↑
bolt402-go     (CGo wrapper calling bolt402-ffi)
```

### Design Principles

1. **Ports & Adapters**: Core business logic has zero external dependencies. Lightning backends and token stores are traits (ports). Implementations are adapters in separate crates.
2. **Domain types in proto**: `L402Challenge`, `L402Token`, `L402Error` live in `bolt402-proto` so both client and server implementations can share them.
3. **Error handling**: `thiserror` for typed errors, no `anyhow` in library code (only in binaries/tests).
4. **Async-first**: All port traits are `async_trait`. Runtime-agnostic where possible, but `tokio` for concrete adapters.

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
- `bolt402-mock` provides a mock L402 server for integration testing
- CI runs: `cargo fmt --check`, `cargo clippy`, `cargo test`, `cargo doc`

### Code Style

- `rustfmt` with project config (see `rustfmt.toml`)
- `clippy` with project config (see `clippy.toml`)
- Documentation on all public items (enforced by `#![warn(missing_docs)]`)
- Examples in doc comments where useful

## For AI Agents (Claude Code, Codex, etc.)

See `CLAUDE.md` for specific instructions when working on this codebase.

## Key Decisions

- **Rust-first**: Core protocol engine in Rust for correctness and multi-language FFI
- **bolt402 name**: Renamed from `lnpay` (already taken). "bolt" references BOLT specs, "402" references HTTP 402
- **Dual license**: MIT OR Apache-2.0 (matches Rust ecosystem convention)
- **Edition 2024**: Using latest Rust edition
- **MSRV 1.85**: Minimum supported Rust version

## References

- [L402 Protocol](https://docs.lightning.engineering/the-lightning-network/l402)
- [Original proposal](../lnpay-proposal.md)
- [Lightning Labs agent-kit](https://github.com/lightninglabs/agent-kit)
- [Aperture (L402 reverse proxy)](https://github.com/lightninglabs/aperture)
