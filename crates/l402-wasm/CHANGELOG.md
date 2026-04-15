# Changelog

All notable changes to `@lightninglabs/l402` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this package adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-04-09

Initial release. The full L402sdk L402 client SDK compiled to WebAssembly via `wasm-pack`.

- `WasmL402Client` with factory methods: `withLndRest()`, `withClnRest()`, `withSwissKnife()`
- Direct backend wrappers: `WasmLndRestBackend`, `WasmClnRestBackend`, `WasmSwissKnifeBackend`
- `WasmBudgetConfig` for per-request, hourly, daily, and total spending limits
- Automatic L402 negotiation, token caching, and receipt tracking
- Browser tests and Node.js integration tests

[0.1.0]: https://github.com/lightninglabs/L402sdk/releases/tag/l402-wasm-v0.1.0
