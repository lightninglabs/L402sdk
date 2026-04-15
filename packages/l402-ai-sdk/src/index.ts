/**
 * l402-ai-sdk: L402 Lightning payment tools for the Vercel AI SDK.
 *
 * All L402 protocol logic runs in Rust via WASM (`l402-wasm`).
 * This package provides Vercel AI SDK tool definitions that wrap the
 * WASM L402 client.
 *
 * @example
 * ```typescript
 * import { createL402Tools, WasmL402Client, WasmBudgetConfig } from '@lightninglabs/l402-ai';
 * import { generateText } from 'ai';
 * import { openai } from '@ai-sdk/openai';
 *
 * const client = WasmL402Client.withLndRest(
 *   'https://localhost:8080',
 *   process.env.LND_MACAROON!,
 *   new WasmBudgetConfig(1000, 0, 50000, 0),
 *   100,
 * );
 *
 * const tools = createL402Tools({ client });
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

export { createL402Tools, type L402ToolsConfig } from './tools.js';

// Re-export WASM runtime values and types so consumers don't need l402-wasm directly
export {
  WasmL402Client,
  WasmL402Response,
  WasmBudgetConfig,
  WasmReceipt,
  WasmLndRestBackend,
  WasmSwissKnifeBackend,
} from '@lightninglabs/l402';
