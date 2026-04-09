# Design Doc 006: Vercel AI SDK Integration

**Status:** Superseded by `docs/design/045-wasm-bindings.md`
**Issue:** #8
**Author:** Dario Anongba Varela

> Historical note: this document captures the original pure-TypeScript proposal. The implemented package no longer ships a native TS `L402Client` or pluggable TS backends. Today `bolt402-ai-sdk` is a thin wrapper around `bolt402-wasm`, and the current public entry point is `createBolt402Tools({ client })` with a `WasmL402Client`.

## Problem

bolt402 provides a Rust L402 client SDK, but the primary consumers of L402-gated APIs today are AI agents, most of which run in TypeScript/Node.js. The Vercel AI SDK is the dominant framework for building AI agent applications in TypeScript (20M+ monthly downloads). There is no existing library that provides Vercel AI SDK tools for L402 payments.

Lightning Labs' `lightning-agent-tools` provides CLI-based tools (lnget) and an MCP server, but no programmatic TypeScript SDK and no Vercel AI SDK integration. bolt402 fills this gap.

## Goals

1. Provide a TypeScript package (`bolt402-ai-sdk`) that gives AI agents the ability to pay for L402-gated APIs
2. Expose Vercel AI SDK tools via a simple `createBolt402Tools()` function
3. Mirror the Rust core's hexagonal architecture: pluggable Lightning backends, token caching, budget tracking
4. Ship with working Lightning backends (LND REST, SwissKnife REST)
5. Include comprehensive tests, docs, and a working example

## Non-Goals

- WASM bindings from the Rust core (future work, separate issue)
- Full BOLT11 invoice decoding in TypeScript (use amount from the challenge or backend response)
- Server-side L402 middleware (this is client-side only)

## Design

### Package Structure

```
packages/
  bolt402-ai-sdk/
    src/
      index.ts              # Public API exports
      l402-client.ts        # L402Client: core protocol engine
      tools.ts              # createBolt402Tools(): Vercel AI SDK tools
      types.ts              # Shared types (LnBackend, TokenStore, etc.)
      token-store.ts        # InMemoryTokenStore adapter
      budget.ts             # BudgetTracker
      receipt.ts            # Receipt type
      backends/
        lnd.ts              # LND REST API backend
        swissknife.ts       # SwissKnife REST API backend
    tests/
      l402-client.test.ts   # Unit tests for L402Client
      tools.test.ts         # Unit tests for AI SDK tools
      budget.test.ts        # Budget tracker tests
      token-store.test.ts   # Token store tests
    package.json
    tsconfig.json
    vitest.config.ts
    README.md
```

### Architecture (Hexagonal, mirroring Rust core)

```
                    Vercel AI SDK
                         │
                  createBolt402Tools()
                         │
                    ┌─────────────┐
                    │  L402Client  │  (core engine)
                    │             │
                    │ - fetch()   │
                    │ - get()     │
                    │ - post()    │
                    └──┬───┬───┬──┘
                       │   │   │
              ┌────────┘   │   └────────┐
              ▼            ▼            ▼
        ┌──────────┐ ┌──────────┐ ┌──────────┐
        │LnBackend │ │TokenStore│ │  Budget   │
        │  (port)  │ │  (port)  │ │ Tracker   │
        └────┬─────┘ └────┬─────┘ └──────────┘
             │             │
     ┌───────┴──────┐     │
     ▼              ▼     ▼
  ┌──────┐   ┌─────────┐ ┌──────────┐
  │ LND  │   │Swissknife│ │InMemory  │
  │ REST │   │  REST    │ │TokenStore│
  └──────┘   └─────────┘ └──────────┘
```

### Core Types (ports)

```typescript
// Lightning backend port
interface LnBackend {
  payInvoice(bolt11: string, maxFeeSats: number): Promise<PaymentResult>;
  getBalance(): Promise<number>;
  getInfo(): Promise<NodeInfo>;
}

// Token storage port
interface TokenStore {
  get(endpoint: string): Promise<CachedToken | null>;
  put(endpoint: string, macaroon: string, preimage: string): Promise<void>;
  remove(endpoint: string): Promise<void>;
  clear(): Promise<void>;
}

interface PaymentResult {
  preimage: string;
  paymentHash: string;
  amountSats: number;
  feeSats: number;
}

interface NodeInfo {
  pubkey: string;
  alias: string;
  numActiveChannels: number;
}
```

### Implemented Surface

The final package is thinner than this original proposal. `bolt402-ai-sdk` does not ship its own TypeScript `L402Client` anymore; it wraps a `WasmL402Client` from `bolt402-wasm`:

```typescript
import { createBolt402Tools } from 'bolt402-ai-sdk';
import init, { WasmBudgetConfig, WasmL402Client } from 'bolt402-wasm';

await init();

const client = WasmL402Client.withLndRest(
  'https://localhost:8080',
  'hex-encoded-admin-macaroon',
  new WasmBudgetConfig(1000, 0, 10000, 0),
  100,
);

const tools = createBolt402Tools({ client });
```

The runtime L402 flow is still the same:
1. Check token cache for the endpoint
2. Make the unauthenticated request
3. Parse any `WWW-Authenticate: L402` challenge
4. Enforce budgets before payment
5. Pay the invoice through the configured backend
6. Cache the token and retry with `Authorization: L402`
7. Record the receipt

### Vercel AI SDK Tools

The implemented package returns two tools:

#### `l402_fetch`
Fetch any URL, automatically handling L402 payment challenges.

#### `l402_get_receipts`
Return accumulated payment receipts for auditing and cost reporting.

No dedicated `l402_get_balance` tool shipped in the final implementation.

### Backend Selection

`bolt402-ai-sdk` no longer exposes native TypeScript backend classes. Backend selection now happens in `bolt402-wasm`:

- `WasmL402Client.withLndRest(...)`
- `WasmL402Client.withSwissKnife(...)`
- `WasmClnRestBackend` for direct CLN REST access from JS/TS when you need the backend wrapper itself

## Key Decisions

1. **WASM-first, Rust as the single source of truth.** The implemented package keeps the protocol engine, budget logic, token cache, and receipts in Rust and exposes them through `wasm-bindgen`.

2. **`packages/` directory in the repo root.** Separates TypeScript packages from the Rust workspace. The Rust crates stay in `crates/`, TypeScript packages go in `packages/`. Clean separation.

3. **Zod for schema validation.** The Vercel AI SDK uses Zod for tool input schemas. It's the standard and required for type inference.

4. **Vitest for testing.** Fast, TypeScript-native, good Vercel AI SDK ecosystem support.

5. **Browser-compatible networking via Rust backends.** The shipped backends use `reqwest`, which compiles to browser `fetch` on WASM targets.

6. **Budget tracking is optional.** Defaults to unlimited if not configured, matching the Rust core behavior.

7. **Small tool surface.** The final package keeps the AI SDK surface to `l402_fetch` and `l402_get_receipts`; approval logic can be layered at the application level.

## Alternatives Considered

- **Native TypeScript client:** Rejected in the final implementation in favor of a single Rust/WASM implementation shared with the rest of the SDK.
- **MCP server instead of Vercel AI SDK tools:** MCP is framework-agnostic but doesn't integrate as tightly with the Vercel AI SDK's tool calling, type inference, and streaming. The Vercel AI SDK is the target framework per the issue.
- **Single mega-tool:** Instead of 3 tools, use a single tool with a discriminated `action` field. Rejected because separate tools give the LLM clearer affordances and better type inference.

## Testing Plan

1. **Unit tests** for L402Client: mock HTTP responses (402 with challenge headers, 200 after payment), mock LnBackend
2. **Unit tests** for each tool: verify schema, mock L402Client, check return values
3. **Unit tests** for backends: mock HTTP, verify correct API calls to LND/SwissKnife
4. **Unit tests** for budget tracker and token store
5. **Integration test** with `bolt402-mock` server: start the mock server, configure L402Client to use it with a mock backend, verify end-to-end flow
6. **CI:** lint (eslint), format (prettier), type-check (tsc), test (vitest)

## Dependencies

- `ai` (Vercel AI SDK core, peer dependency)
- `zod` (schema validation, peer dependency)
- No other runtime dependencies (uses native `fetch`)

## Future Work

- WASM bindings to replace native TS core (keep same API surface)
- Additional tools: `l402_pay_invoice` (direct invoice payment), `l402_create_invoice` (for receiving)
- More backends: CLN, Phoenixd, custom REST
- npm publish pipeline in CI
