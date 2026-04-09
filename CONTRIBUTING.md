# Contributing to bolt402

Thanks for your interest in contributing to bolt402! This guide covers everything you need to get started.

## Prerequisites

- **Rust** (stable toolchain, MSRV 1.85) — [rustup.rs](https://rustup.rs/)
- **Node.js 22+** — only needed for TypeScript packages (`packages/bolt402-ai-sdk`)
- **protobuf-compiler** — required for the LND gRPC adapter (`bolt402-lnd`)
  - macOS: `brew install protobuf`
  - Ubuntu/Debian: `sudo apt-get install -y protobuf-compiler`
  - Arch: `sudo pacman -S protobuf`

## Getting Started

```bash
# Clone the repository
git clone https://github.com/lightninglabs/bolt402.git
cd bolt402

# Build all Rust crates
cargo build --workspace

# Run all Rust tests
cargo test --workspace

# Check formatting and lints
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# Build documentation
cargo doc --workspace --no-deps
```

For the TypeScript package:

```bash
cd packages/bolt402-ai-sdk
yarn install
yarn typecheck
yarn test
```

Or use the Makefile shortcuts:

```bash
make check    # fmt + lint + test
make ci       # full CI pipeline (fmt + lint + test + doc)
```

## Project Structure

```
bolt402/
├── crates/
│   ├── bolt402-proto/       # L402 protocol types, parsing, token construction
│   ├── bolt402-core/        # Client engine, ports (traits), adapters
│   ├── bolt402-lnd/         # LND backends (gRPC + REST)
│   ├── bolt402-cln/         # CLN backends (gRPC + REST)
│   ├── bolt402-nwc/         # Nostr Wallet Connect backend adapter
│   ├── bolt402-swissknife/  # SwissKnife REST backend adapter
│   ├── bolt402-mock/        # Mock L402 server for testing
│   ├── bolt402-sqlite/      # SQLite token store
│   ├── bolt402-ffi/         # C-compatible FFI layer
│   ├── bolt402-python/      # Python bindings
│   └── bolt402-wasm/        # WebAssembly bindings
├── packages/
│   ├── bolt402-ai-sdk/      # Vercel AI SDK integration (TypeScript)
│   └── bolt402-langchain/   # LangChain integration (Python package)
├── docs/
│   └── design/              # Design documents for each feature
├── AGENTS.md                # Architecture overview
├── CLAUDE.md                # AI agent coding instructions
├── CONTRIBUTING.md          # This file
└── CHANGELOG.md             # Release history
```

The architecture is hexagonal (ports and adapters). See [AGENTS.md](AGENTS.md) for a detailed breakdown of the design, crate dependency graph, and key decisions.

## Coding Standards

### Rust

- **Formatting**: `cargo fmt` with the project's `rustfmt.toml` configuration.
- **Linting**: `cargo clippy` with pedantic lints enabled (see `clippy.toml` and workspace `Cargo.toml`).
- **Documentation**: All public items must have doc comments. `#![warn(missing_docs)]` is enforced at the workspace level.
- **Error handling**: Use `thiserror` for typed errors. No `unwrap()` in library code. No `anyhow` in library crates.
- **Async**: Port traits use `async_trait`. Concrete adapters use `tokio`.
- **Imports**: Group as std → external crates → internal crates.
- **No unsafe**: Unless there is an extremely compelling reason.

### TypeScript

- **Type checking**: `tsc --noEmit` must pass.
- **Testing**: `vitest` for unit tests.
- **Style**: Follow the existing code patterns in `packages/bolt402-ai-sdk`.

See [CLAUDE.md](CLAUDE.md) for the complete coding rules.

## Making Changes

### 1. Open an Issue First

Before starting work, [open an issue](https://github.com/lightninglabs/bolt402/issues/new/choose) to discuss the change. This avoids duplicate work and ensures the change aligns with the project direction.

### 2. Create a Feature Branch

Branch from `main` with a descriptive name:

```bash
git checkout main
git pull origin main
git checkout -b type/short-description
```

Branch name prefixes: `feat/`, `fix/`, `docs/`, `test/`, `refactor/`, `chore/`, `ci/`.

### 3. Implement the Change

- Write clean, production-quality code. No TODOs, no placeholders.
- Add tests for new functionality.
- Update documentation if applicable.

### 4. Run CI Locally

Before pushing, make sure everything passes:

```bash
make ci
```

This runs formatting checks, clippy, tests, and doc builds — the same checks as GitHub Actions.

### 5. Commit with Conventional Commits

Use [conventional commit](https://www.conventionalcommits.org/) messages:

```
feat: add NWC backend adapter
fix: handle empty macaroon in L402 challenge
docs: update README with quick start guide
test: add integration tests for budget tracker
refactor: extract token validation into separate module
chore: update dependencies
ci: add code coverage reporting
```

One logical change per commit. Keep commits atomic and self-contained.

### 6. Open a Pull Request

- Reference the related issue: `Closes #123` or `Refs #123`.
- Describe what changed, why, and how to test it.
- PRs are squash-merged into a single clean commit on `main`.

### 7. CI Must Pass

All of these must be green before merge:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo doc --workspace --no-deps` (with `-D warnings` for rustdoc)
- TypeScript: `tsc --noEmit` and `vitest run` (for `bolt402-ai-sdk`)

## Testing

### Unit Tests

Each Rust module should have inline tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        // ...
    }
}
```

### Integration Tests

The `bolt402-mock` crate provides a mock L402 server for integration testing. See `crates/bolt402-mock/tests/` for examples.

For the Docker-based regtest suite against Aperture:

```bash
make regtest-up
make regtest-init
make regtest-test
make regtest-down
```

### TypeScript Tests

```bash
cd packages/bolt402-ai-sdk
yarn test
```

## Design Documents

For non-trivial changes, write a design document in `docs/design/` before implementing. Follow the existing format (see `docs/design/001-l402-client.md` for an example). Design docs include:

- Problem statement
- Proposed design
- API sketch
- Key decisions and alternatives
- Testing plan

## License

By contributing, you agree that your contributions will be licensed under the project's dual license: MIT OR Apache-2.0.

## Questions?

Open a [GitHub issue](https://github.com/lightninglabs/bolt402/issues) for bugs and feature requests. For general questions and discussion, start a conversation in the issue tracker.
