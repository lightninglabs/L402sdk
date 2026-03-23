/**
 * bolt402-ai-sdk: L402 Lightning payment tools for the Vercel AI SDK.
 *
 * Let AI agents autonomously pay for L402-gated APIs using the Lightning Network.
 *
 * @example
 * ```typescript
 * import { createBolt402Tools, LndBackend } from 'bolt402-ai-sdk';
 * import { generateText } from 'ai';
 * import { openai } from '@ai-sdk/openai';
 *
 * const tools = createBolt402Tools({
 *   backend: new LndBackend({
 *     url: 'https://localhost:8080',
 *     macaroon: process.env.LND_MACAROON!,
 *   }),
 *   budget: { perRequestMax: 1000, dailyMax: 50000 },
 * });
 *
 * const result = await generateText({
 *   model: openai('gpt-4o'),
 *   tools,
 *   prompt: 'Fetch the premium data from https://api.example.com/v1/data',
 * });
 * ```
 *
 * @module
 */

// Core
export { L402Client, L402Error, parseL402Challenge } from './l402-client.js';
export { createBolt402Tools, type Bolt402ToolsConfig } from './tools.js';
export { InMemoryTokenStore } from './token-store.js';
export { LocalStorageTokenStore } from './local-storage-token-store.js';
export { FileTokenStore } from './file-token-store.js';
export { BudgetTracker, BudgetExceededError } from './budget.js';

// Backends
export { LndBackend, type LndBackendConfig } from './backends/lnd.js';
export { SwissKnifeBackend, type SwissKnifeBackendConfig } from './backends/swissknife.js';

// Types
export type {
  Budget,
  CachedToken,
  L402Challenge,
  L402ClientConfig,
  L402Response,
  LnBackend,
  NodeInfo,
  PaymentResult,
  Receipt,
  TokenStore,
} from './types.js';
