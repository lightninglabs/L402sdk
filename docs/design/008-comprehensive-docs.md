# Design Doc 008: Comprehensive Documentation and Tutorials

**Issue:** #17
**Author:** Dario Anongba Varela
**Date:** 2026-03-16

## Problem

L402sdk has working code across five Rust crates and a TypeScript AI SDK package, but the documentation doesn't match the implementation reality. The README still shows several items as "not started" that are complete. There are no standalone examples, no architecture guide, no tutorial for implementing custom backends, and no guide for budget control. The rustdoc is solid (every public item is documented), but there's no narrative documentation that walks a developer through real use cases.

## Proposed Changes

### 1. Update README.md

Bring the root README in sync with reality:
- Update the crate status table (everything is implemented now)
- Add `l402-swissknife` and `l402-ai-sdk` to the crate table
- Update the architecture diagram to include SwissKnife and AI SDK
- Fix the quick start code to match the current API
- Update the roadmap to reflect completed items
- Add links to documentation and examples

### 2. Architecture Guide (`docs/architecture.md`)

A standalone document explaining:
- Hexagonal/ports-and-adapters design philosophy
- Crate dependency graph (with `l402-swissknife` and `l402-ai-sdk`)
- Port definitions: `LnBackend`, `TokenStore`
- The L402 protocol flow (402 → parse challenge → pay → cache → retry)
- How adapters plug in (LND, SwissKnife, custom)
- Rust ↔ TypeScript architecture symmetry

### 3. Tutorial: Getting Started with l402-mock (`docs/tutorials/getting-started.md`)

Minimal tutorial for first-time users:
- Add `l402-core` and `l402-mock` as deps
- Create a mock server with protected endpoints
- Build an `L402Client` with the mock backend
- Make requests and observe the 402 → pay → 200 flow
- Inspect receipts and spending
- No real Lightning node needed

### 4. Tutorial: Custom Lightning Backend (`docs/tutorials/custom-backend.md`)

Shows how to implement `LnBackend` for a new Lightning implementation:
- Explanation of the `LnBackend` trait
- Step-by-step implementation of a hypothetical CLN backend
- Testing with `l402-mock`
- Integration testing patterns

### 5. Tutorial: Budget Control for Autonomous Agents (`docs/tutorials/budget-control.md`)

Shows how to configure spending limits:
- Per-request, hourly, daily, total limits
- Domain-specific budgets
- Receipt inspection for cost analysis
- Patterns for production use with AI agents

### 6. Rust Examples (`examples/`)

- `examples/basic-mock/main.rs` — Self-contained CLI demo using `l402-mock` (extract and improve from the existing demo)
- `examples/budget-control/main.rs` — Demonstrates budget limits and rejection

### 7. TypeScript Example (`examples/ai-agent/`)

- `examples/ai-agent/README.md` — How to set up and run
- `examples/ai-agent/index.ts` — Vercel AI SDK + l402-ai-sdk integration example

## Key Decisions

- **No mdBook or documentation site** for now. GitHub markdown is sufficient at this stage. We can add a doc site later when there's more content.
- **Examples in `examples/` directory** at workspace root for Rust (standard Cargo convention), separate directory for TypeScript.
- **Tutorials in `docs/tutorials/`** for narrative documentation.
- **Architecture in `docs/architecture.md`** as the technical reference.

## Alternatives Considered

- **mdBook documentation site**: Too much infrastructure overhead for now. Markdown files in `docs/` are discoverable on GitHub and easy to maintain.
- **Rustdoc-only approach**: Rustdoc is great for API reference but poor for narrative tutorials and architecture explanations.

## Testing Plan

- Verify all code examples in tutorials compile (use `cargo test --doc` where possible)
- Verify Rust examples compile: `cargo build --examples`
- Manual review of documentation flow and accuracy
- Ensure CI still passes with new example crates

## Scope

This PR focuses on documentation and examples only. No code changes to the library crates.
