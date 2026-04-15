/**
 * Shared L402 client singleton.
 *
 * Both the chat route and the l402-fetch route use this so that receipts
 * accumulate in a single place and can be queried via /api/l402-receipts.
 *
 * Requires a real Lightning backend (LND or SwissKnife) configured via
 * environment variables. See .env.example for details.
 */

import { WasmL402Client, WasmBudgetConfig } from '@lightninglabs/l402-ai';

function createClient(): WasmL402Client {
  const backendType = process.env.BACKEND_TYPE;

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

  throw new Error(
    'No Lightning backend configured. Set BACKEND_TYPE to "lnd" or "swissknife" ' +
      'and provide the required credentials in .env.local. See .env.example for details.',
  );
}

let sharedClient: WasmL402Client | null = null;

/** Get the shared L402 client (creates on first call). */
export function getSharedL402Client(): WasmL402Client {
  if (!sharedClient) {
    sharedClient = createClient();
  }
  return sharedClient;
}
