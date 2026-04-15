# Design: Integration Test Suite

**Issue:** #5
**Author:** Dario Anongba Varela
**Date:** 2026-03-15
**Status:** Implemented (Tier 1 + Tier 2)

## Problem

L402sdk has unit tests for individual modules (proto parsing, cache, budget, mock server) but no end-to-end tests that exercise the full L402 flow through the `L402Client`. We need to verify that all components work together: HTTP request → 402 challenge → invoice payment → token construction → authenticated retry → success.

## Proposed Design

### Tier 1: Mock-based Integration Tests (CI)

A workspace-level `tests/` directory with integration tests that spin up `l402-mock::MockL402Server` and wire it to `L402Client` via `MockLnBackend`. These test the real HTTP flow over localhost without any Lightning infrastructure.

**Test cases:**

1. **Happy path** — GET request to protected endpoint: 402 → pay → 200 with correct body
2. **POST with body** — Same flow with POST method and JSON body
3. **Token caching** — Second request to same endpoint uses cached token, no payment
4. **Budget per-request limit** — Payment exceeding per-request budget is rejected
5. **Budget total limit** — Cumulative spending exceeding total budget is rejected
6. **Insufficient balance** — Mock backend with low balance rejects payment
7. **Multiple endpoints** — Different protected endpoints get separate tokens
8. **Cache eviction** — Evicted tokens trigger re-payment
9. **Invalid token rejection** — Manually corrupted cached token triggers re-payment
10. **Non-402 passthrough** — Unprotected endpoint returns directly (no payment)
11. **404 passthrough** — Unknown endpoint returns 404 without payment
12. **Receipts** — Verify receipt recording (amount, hash, status, latency)
13. **Custom response body** — Endpoint with custom body returns it correctly

### Demo Binary

`examples/demo.rs` that demonstrates the full flow interactively with step-by-step output via `tracing`. Shows: server startup, initial request, 402 challenge, payment, retry, success, cached re-request, receipt summary.

### File Layout

```
crates/l402-mock/
  tests/
    integration.rs        # All Tier 1 tests
  examples/
    demo.rs               # Interactive demo binary
```

## Key Decisions

- **Tests in l402-mock crate**: Integration tests live in `l402-mock/tests/` because the mock crate already depends on both `l402-core` and `l402-proto`, providing access to all components without extra dependencies. Workspace-root `tests/` doesn't work in Cargo workspaces (no owning package).
- **Single test file**: All integration tests in one file to share helper setup code and keep CI fast (one test binary).
- **Demo as mock example**: The demo binary lives in `l402-mock/examples/` since it demonstrates the mock server + client workflow.
- **Tier 2 is now implemented**: Docker/regtest coverage lives in `tests/regtest/` and exercises the full Aperture-backed flow across LND gRPC, LND REST, CLN gRPC, and CLN REST.

## Alternatives Considered

- **Workspace-level tests/**: Doesn't compile in Cargo workspaces; rejected.
- **Separate test crate**: Adds complexity for little benefit; l402-mock already has the right deps.

## Testing Plan

- `cargo test -p l402-mock --test integration` runs all Tier 1 tests
- `cargo run -p l402-mock --example demo` runs the interactive demo
- `make regtest-up && make regtest-init && make regtest-test` runs the Tier 2 Docker/Aperture suite
- CI already runs `cargo test` which picks up all tests including integration
