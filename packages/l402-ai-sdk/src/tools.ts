/**
 * Vercel AI SDK tool definitions for L402 payments.
 *
 * All L402 protocol logic runs in Rust via WASM — no TypeScript
 * reimplementation. This module provides thin tool wrappers around
 * the `WasmL402Client` from `l402-wasm`.
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
 *   prompt: 'Fetch data from https://api.example.com/paid-endpoint',
 * });
 * ```
 */

import { tool } from 'ai';
import { z } from 'zod';

import type { WasmL402Client } from '@lightninglabs/l402';

/** Configuration for creating L402sdk AI SDK tools. */
export interface L402ToolsConfig {
  /** A `WasmL402Client` instance from `l402-wasm`. */
  client: WasmL402Client;
}

/**
 * Create Vercel AI SDK tools for L402 Lightning payments.
 *
 * Returns an object with tools that can be spread into the `tools` parameter
 * of `generateText` or `streamText`.
 *
 * Tools provided:
 * - `l402_fetch`: Fetch a URL, automatically paying L402 challenges
 * - `l402_get_receipts`: Get payment receipts for cost tracking
 */
export function createL402Tools(config: L402ToolsConfig) {
  const { client } = config;

  return {
    l402_fetch: tool({
      description:
        'Fetch a URL, automatically paying Lightning invoices for L402-gated APIs. ' +
        'When the server requires payment (HTTP 402), this tool pays the Lightning invoice, ' +
        'caches the token, and retries the request. Returns the response body and payment ' +
        'receipt if a payment was made. Use this for any API that requires L402 authentication.',
      inputSchema: z.object({
        url: z.string().url().describe('The URL to fetch'),
        method: z
          .enum(['GET', 'POST'])
          .default('GET')
          .describe('HTTP method to use'),
        body: z
          .string()
          .optional()
          .describe('Request body (for POST requests). Should be JSON-encoded.'),
      }),
      execute: async ({ url, method, body }) => {
        try {
          const response =
            method === 'POST'
              ? await client.post(url, body ?? undefined)
              : await client.get(url);

          const receipt = response.receipt;

          return {
            status: response.status,
            body: response.body,
            paid: response.paid,
            receipt: receipt
              ? {
                  amountSats: Number(receipt.amountSats),
                  feeSats: Number(receipt.feeSats),
                  totalCostSats: Number(receipt.totalCostSats()),
                  paymentHash: receipt.paymentHash,
                }
              : null,
          };
        } catch (e) {
          return {
            status: 0,
            body: '',
            paid: false,
            receipt: null,
            error: e instanceof Error ? e.message : String(e),
          };
        }
      },
    }),

    l402_get_receipts: tool({
      description:
        'Get all L402 payment receipts from this session. ' +
        'Useful for tracking costs, auditing payments, and reporting spend to the user.',
      inputSchema: z.object({}),
      execute: async () => {
        try {
          const totalSpent = await client.totalSpent();
          const receipts = await client.receipts();

          return {
            totalSpentSats: Number(totalSpent),
            paymentCount: Array.isArray(receipts) ? receipts.length : 0,
            receipts: Array.isArray(receipts)
              ? receipts.map((r: any) => ({
                  endpoint: r.endpoint,
                  amountSats: Number(r.amountSats),
                  feeSats: Number(r.feeSats),
                  totalCostSats: Number(r.totalCostSats()),
                  responseStatus: r.responseStatus,
                  timestamp: Number(r.timestamp),
                }))
              : [],
          };
        } catch (e) {
          return {
            totalSpentSats: 0,
            paymentCount: 0,
            receipts: [],
            error: e instanceof Error ? e.message : String(e),
          };
        }
      },
    }),
  };
}
