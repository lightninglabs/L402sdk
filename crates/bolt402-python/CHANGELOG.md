# Changelog

All notable changes to `bolt402` (Python) will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this package adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-04-09

Initial release. The full bolt402 L402 client SDK as Python bindings via PyO3.

- `L402Client` with factory methods: `with_lnd_rest()`, `with_cln_rest()`, `with_swissknife()`
- `Budget` for per-request, hourly, daily, and total spending limits
- `LndRestBackend`, `ClnRestBackend`, `SwissKnifeBackend` direct backend wrappers
- Automatic L402 negotiation, token caching, and receipt tracking
- Wheels for linux x86_64/aarch64, macOS x86_64/aarch64, windows x86_64

[0.1.0]: https://github.com/lightninglabs/bolt402/releases/tag/bolt402-python-v0.1.0
