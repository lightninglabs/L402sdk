<div align="center">
  <h1>l402-ai-sdk</h1>

  <p>
    <strong>L402 Lightning payment tools for the Vercel AI SDK</strong>
  </p>

  <p>
    <a href="https://www.npmjs.com/package/@lightninglabs/l402-ai"><img alt="npm" src="https://img.shields.io/npm/v/@lightninglabs/l402-ai.svg"/></a>
    <a href="https://www.npmjs.com/package/@lightninglabs/l402-ai"><img alt="npm downloads" src="https://img.shields.io/npm/dm/@lightninglabs/l402-ai.svg"/></a>
    <a href="https://github.com/lightninglabs/L402sdk/blob/main/LICENSE-MIT"><img alt="MIT or Apache-2.0 Licensed" src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg"/></a>
  </p>

</div>

Let AI agents autonomously pay for APIs with Bitcoin over the Lightning Network.

All L402 protocol logic runs in Rust via WASM (`l402-wasm`). This package provides thin Vercel AI SDK tool wrappers.

## What is L402?

[L402](https://docs.lightning.engineering/the-lightning-network/l402) is a protocol that uses HTTP 402 (Payment Required) responses to gate API access behind Lightning Network payments. When a server responds with 402, the client pays a Lightning invoice and retries with proof of payment.

l402-ai-sdk wraps this flow into [Vercel AI SDK tools](https://ai-sdk.dev/docs/ai-sdk-core/tools-and-tool-calling), so AI agents can access paid APIs without manual intervention.

## Install

```bash
yarn add @lightninglabs/l402-ai
```

Peer dependencies: `ai` (>=6.0.0) and `zod` (^3.25 or ^4.1).

## Quick Start

```typescript
import { createL402Tools, WasmL402Client, WasmBudgetConfig } from '@lightninglabs/l402-ai';
import { generateText } from 'ai';
import { openai } from '@ai-sdk/openai';

// Create L402 client backed by LND REST
const client = WasmL402Client.withLndRest(
  'https://localhost:8080',
  process.env.LND_MACAROON!,
  new WasmBudgetConfig(1000, 0, 50_000, 0),  // per-request, hourly, daily, total
  100,  // max routing fee (sats)
);

// Create AI SDK tools
const tools = createL402Tools({ client });

// Use with any Vercel AI SDK model
const result = await generateText({
  model: openai('gpt-4o'),
  tools,
  maxSteps: 5,
  prompt: 'Fetch the premium weather data from https://api.example.com/v1/weather',
});

console.log(result.text);
```

## Tools

`createL402Tools()` returns two tools:

### `l402_fetch`

Fetch any URL, automatically handling L402 payment challenges. When the server returns HTTP 402 with a Lightning invoice, the tool pays it, caches the token, and retries.

**Parameters:**
- `url` (string, required): The URL to fetch
- `method` (string, optional): HTTP method (GET or POST). Default: GET
- `body` (string, optional): Request body for POST (JSON-encoded)

**Returns:** Response body, status code, payment flag, and receipt (if paid).

### `l402_get_receipts`

Get all payment receipts from the current session for cost tracking and auditing.

**Returns:** Total spent, payment count, and detailed receipts.

## Lightning Backends

Backends are configured when creating the `WasmL402Client` (all in Rust/WASM):

### LND REST

```typescript
import { WasmL402Client, WasmBudgetConfig } from '@lightninglabs/l402-ai';

const client = WasmL402Client.withLndRest(
  'https://localhost:8080',
  'hex-encoded-admin-macaroon',
  WasmBudgetConfig.unlimited(),
  100,
);
```

### SwissKnife

```typescript
const client = WasmL402Client.withSwissKnife(
  'https://api.numeraire.tech',
  'your-api-key',
  WasmBudgetConfig.unlimited(),
  100,
);
```

## Budget Control

Set spending limits via `WasmBudgetConfig` to prevent runaway costs. Pass `0` for any limit to leave it unlimited.

```typescript
const budget = new WasmBudgetConfig(
  1000,       // per-request max (sats)
  10_000,     // hourly max (sats)
  100_000,    // daily max (sats)
  1_000_000,  // total max (sats)
);

const client = WasmL402Client.withLndRest(url, macaroon, budget, 100);
```

## Architecture

```
Vercel AI SDK → createL402Tools() → WasmL402Client (WASM)
                                            │
                                   ┌────────┴────────┐
                                   ▼                  ▼
                              l402-core        l402-proto
                              (Rust/WASM)         (Rust/WASM)
                                   │
                     ┌─────────────┼─────────────┐
                     ▼             ▼             ▼
                  LnBackend    TokenStore    BudgetTracker
                  (port)       (port)
                     │             │
               ┌─────┴─────┐      │
               ▼           ▼      ▼
            LND REST   SwissKnife  InMemory
            (Rust)     (Rust)      (Rust)
```

All protocol logic (challenge parsing, budget enforcement, token caching, receipt tracking) runs in Rust compiled to WASM. The TypeScript layer is a thin wrapper providing Vercel AI SDK tool definitions.

## License

MIT OR Apache-2.0
