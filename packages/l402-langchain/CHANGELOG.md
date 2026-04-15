# Changelog

All notable changes to `l402-langchain` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this package adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-04-09

Initial release. LangChain integration for the L402sdk L402 client SDK.

- `L402FetchTool` — LangChain tool for L402-gated API requests with automatic payment
- `L402BudgetTool` — spending tracker with per-endpoint breakdown
- `PaymentCallbackHandler` — LangChain callback for payment observability and alerts
- `create_l402_client()` — factory supporting LND, CLN, and SwissKnife backends

[0.1.0]: https://github.com/lightninglabs/L402sdk/releases/tag/l402-langchain-v0.1.0
