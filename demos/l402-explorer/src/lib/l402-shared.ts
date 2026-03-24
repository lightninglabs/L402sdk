/**
 * Shared L402 client singleton.
 *
 * Both the chat route and the l402-fetch route use this so that receipts
 * accumulate in a single place and can be queried via /api/l402-receipts.
 *
 * For real backends (LND, SwissKnife), uses the Rust WASM L402 client.
 * For mock mode, uses a TypeScript mock that simulates L402 payments.
 */

import { WasmL402Client, WasmBudgetConfig } from 'bolt402-wasm';
import { MockL402Client } from '@/lib/mock-backend';

/** Common client type — either the real WASM client or the TS mock. */
export type L402ClientLike = WasmL402Client | MockL402Client;

function createClient(): L402ClientLike {
  const backendType = process.env.BACKEND_TYPE || 'mock';

  if (backendType === 'lnd' && process.env.LND_URL && process.env.LND_MACAROON) {
    return WasmL402Client.withLndRest(
      process.env.LND_URL,
      process.env.LND_MACAROON,
      new WasmBudgetConfig(1000n, 0n, 50000n, 0n),
      100n,
    );
  }

  if (
    backendType === 'swissknife' &&
    process.env.SWISSKNIFE_URL &&
    process.env.SWISSKNIFE_API_KEY
  ) {
    return WasmL402Client.withSwissKnife(
      process.env.SWISSKNIFE_URL,
      process.env.SWISSKNIFE_API_KEY,
      new WasmBudgetConfig(1000n, 0n, 50000n, 0n),
      100n,
    );
  }

  return new MockL402Client();
}

let sharedClient: L402ClientLike | null = null;

/** Get the shared L402 client (creates on first call). */
export function getSharedL402Client(): L402ClientLike {
  if (!sharedClient) {
    sharedClient = createClient();
  }
  return sharedClient;
}
