# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] — 2026-10-29

### Changed

- Budgets include routing fees and concurrent payment reservations. Domain limits cannot relax global limits, and unknown payment outcomes block automatic repayment.
- Automatic payments require a backend that enforces fee limits before submission. A zero fee limit allows only zero-fee routes.
- Native LND and CLN REST connections verify TLS certificates and require explicit trust for self-signed certificates. Credential-bearing redirects are disabled.
- Updated Rust, Python, and JavaScript dependencies, CI toolchains, and integration-test images.
- The AI SDK and LangChain packages require the corresponding 0.2 SDK release.

### Fixed

- Reject invalid or amountless invoices before calling the wallet, and check arithmetic used for invoice amounts and budget accounting.
- Preserve payment receipts and recoverable tokens when persistence or subsequent HTTP requests fail.
- Fix FFI error-string ownership and handling of empty endpoint arrays.

### Added

- Regression coverage for fee limits, fee-inclusive budgets, concurrent payments, and receipt preservation, plus a bounded protocol-parser fuzz check.

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

[Unreleased]: https://github.com/lightninglabs/L402sdk/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/lightninglabs/L402sdk/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/lightninglabs/L402sdk/releases/tag/v0.1.0
