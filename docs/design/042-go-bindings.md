# Design Doc: Go Bindings via CGo FFI

**Issue:** #42
**Author:** Dario Anongba Varela
**Date:** 2026-03-20

## Problem

L402sdk is a Rust-first L402 client SDK. To reach Go developers and integrate with Go-based AI agent frameworks (notably Lightning Labs' [lightning-agent-tools](https://github.com/lightninglabs/lightning-agent-tools)), we need native Go bindings.

## Proposed Design

Two-layer architecture, following the same pattern as our Python bindings (PyO3):

```
l402-core (Rust)
      ↓
l402-ffi (Rust cdylib/staticlib, extern "C")
      ↓  C ABI
l402-go (Go package via CGo)
      ↓
Go applications / AI agents
```

### Layer 1: `l402-ffi` (Rust crate)

A new workspace member that exposes l402-core through a C-compatible ABI:

- **Opaque pointers** for `MockServer`, `Client`, `Response`
- **Thread-local error** via `l402_last_error_message()`
- **Shared tokio runtime** for async-to-sync bridging
- **cbindgen** auto-generates `include/L402sdk.h`
- **String ownership**: returned strings freed with `l402_string_free()`

API surface:
- `l402_mock_server_new/url/free`
- `l402_client_new_mock/free`
- `l402_client_get/post`
- `l402_response_status/paid/body/has_receipt/receipt_*/free`
- `l402_client_total_spent/receipts_json`
- `l402_last_error_message/string_free`

### Layer 2: `l402-go` (Go package)

Idiomatic Go wrapper in `bindings/l402-go/`:

- `MockServer` — wraps `L402MockServer*`, finalizer-protected
- `Client` — wraps `L402Client*`, finalizer-protected
- `Response` — pure Go struct (status, paid, body, receipt)
- `Receipt` — pure Go struct with JSON tags for deserialization
- CGo links against the static library (`libl402_ffi.a`)

### CI Integration

New GitHub Actions job `go` that:
1. Builds `l402-ffi` as a static library
2. Copies `libl402_ffi.a` to `bindings/l402-go/lib/`
3. Runs `go test -v ./...`

### Workspace Changes

- Add `l402-ffi` to `workspace.members` (but NOT `default-members` since it requires protobuf/cbindgen to build)
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

- 4 FFI unit tests in Rust (`l402-ffi/src/lib.rs`)
- 10 Go integration tests (`l402-go/l402_test.go`)
- CI job validates the full chain: Rust build → static lib → CGo → Go tests

## Scope

This PR delivers:
- [x] `l402-ffi` crate added to workspace
- [x] `l402-go` package with idiomatic Go API
- [x] Go tests (mock server lifecycle, client lifecycle, GET with payment, POST, receipts, error cases)
- [x] CI job for Go bindings
- [x] README with usage examples
- [x] Auto-generated C header via cbindgen
