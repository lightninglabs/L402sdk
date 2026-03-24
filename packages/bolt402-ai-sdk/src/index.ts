/**
 * bolt402-ai-sdk: L402 Lightning payment tools for the Vercel AI SDK.
 *
 * All L402 protocol logic runs in Rust via WASM (`bolt402-wasm`).
 * This package provides Vercel AI SDK tool definitions that wrap the
 * WASM L402 client.
 *
 * @example
 * ```typescript
 * import { createBolt402Tools } from 'bolt402-ai-sdk';
 * import init, { WasmL402Client, WasmBudgetConfig } from 'bolt402-wasm';
 * import { generateText } from 'ai';
 * import { openai } from '@ai-sdk/openai';
 *
 * await init();
 *
 * const client = WasmL402Client.withLndRest(
 *   'https://localhost:8080',
 *   process.env.LND_MACAROON!,
 *   new WasmBudgetConfig(1000, 0, 50000, 0),
 *   100,
 * );
 *
 * const tools = createBolt402Tools({ client });
 *
 * const result = await generateText({
 *   model: openai('gpt-4o'),
 *   tools,
 *   maxSteps: 5,
 *   prompt: 'Fetch the premium data from https://api.example.com/v1/data',
 * });
 * ```
 *
 * @module
 */

export { createBolt402Tools, type Bolt402ToolsConfig } from './tools.js';

// Re-export WASM types for convenience
export type {
  WasmL402Client,
  WasmL402Response,
  WasmBudgetConfig,
  WasmReceipt,
  WasmLndRestBackend,
  WasmSwissKnifeBackend,
} from 'bolt402-wasm';
