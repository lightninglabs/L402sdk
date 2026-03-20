# Design Doc: Go Bindings via CGo FFI

**Issue:** #42
**Author:** Toshi (maintainer)
**Date:** 2026-03-20

## Problem

bolt402 is a Rust-first L402 client SDK. To reach Go developers and integrate with Go-based AI agent frameworks (notably Lightning Labs' [agent-kit](https://github.com/lightninglabs/agent-kit)), we need native Go bindings.

## Proposed Design

Two-layer architecture, following the same pattern as our Python bindings (PyO3):

```
bolt402-core (Rust)
      ↓
bolt402-ffi (Rust cdylib/staticlib, extern "C")
      ↓  C ABI
bolt402-go (Go package via CGo)
      ↓
Go applications / AI agents
```

### Layer 1: `bolt402-ffi` (Rust crate)

A new workspace member that exposes bolt402-core through a C-compatible ABI:

- **Opaque pointers** for `MockServer`, `Client`, `Response`
- **Thread-local error** via `bolt402_last_error_message()`
- **Shared tokio runtime** for async-to-sync bridging
- **cbindgen** auto-generates `include/bolt402.h`
- **String ownership**: returned strings freed with `bolt402_string_free()`

API surface:
- `bolt402_mock_server_new/url/free`
- `bolt402_client_new_mock/free`
- `bolt402_client_get/post`
- `bolt402_response_status/paid/body/has_receipt/receipt_*/free`
- `bolt402_client_total_spent/receipts_json`
- `bolt402_last_error_message/string_free`

### Layer 2: `bolt402-go` (Go package)

Idiomatic Go wrapper in `bindings/bolt402-go/`:

- `MockServer` — wraps `Bolt402MockServer*`, finalizer-protected
- `Client` — wraps `Bolt402Client*`, finalizer-protected
- `Response` — pure Go struct (status, paid, body, receipt)
- `Receipt` — pure Go struct with JSON tags for deserialization
- CGo links against the static library (`libbolt402_ffi.a`)

### CI Integration

New GitHub Actions job `go` that:
1. Builds `bolt402-ffi` as a static library
2. Copies `libbolt402_ffi.a` to `bindings/bolt402-go/lib/`
3. Runs `go test -v ./...`

### Workspace Changes

- Add `bolt402-ffi` to `workspace.members` (but NOT `default-members` since it requires protobuf/cbindgen to build)
- Add `cbindgen` as workspace build-dependency

## Key Decisions

| Decision | Rationale |
|----------|-----------|
| Static library (`.a`) over shared (`.so`) | Simpler distribution, no runtime LD_LIBRARY_PATH |
| Thread-local error pattern | Safe for concurrent CGo calls from different goroutines |
| Shared tokio runtime | Single runtime avoids per-call overhead |
| JSON for receipts | Avoids complex C struct arrays, Go has excellent JSON support |
| Exclude from default-members | Prevents `cargo build` from failing without cbindgen/protobuf |

## Alternatives Considered

1. **Pure Go implementation**: Would duplicate the Rust core logic. Rejected because the project goal is Rust-first with FFI bindings.
2. **gRPC bridge**: Overkill for an in-process SDK. Adds latency and a server process.
3. **WASM**: Go's WASM support is immature for this use case. CGo is the standard approach.

## Testing Plan

- 4 FFI unit tests in Rust (`bolt402-ffi/src/lib.rs`)
- 10 Go integration tests (`bolt402-go/bolt402_test.go`)
- CI job validates the full chain: Rust build → static lib → CGo → Go tests

## Scope

This PR delivers:
- [x] `bolt402-ffi` crate added to workspace
- [x] `bolt402-go` package with idiomatic Go API
- [x] Go tests (mock server lifecycle, client lifecycle, GET with payment, POST, receipts, error cases)
- [x] CI job for Go bindings
- [x] README with usage examples
- [x] Auto-generated C header via cbindgen
