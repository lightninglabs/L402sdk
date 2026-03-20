/**
 * Bolt402 tools for AI SDK v6.
 *
 * The bolt402-ai-sdk package's createBolt402Tools() uses ai v4's tool() API,
 * which produces schemas incompatible with ai v6 (type: "None" error).
 * This wrapper uses ai v6's tool() directly with the L402Client from bolt402.
 */

// eslint-disable-next-line @typescript-eslint/no-explicit-any
import { tool } from 'ai';
import { z } from 'zod';
import { L402Client, InMemoryTokenStore, type LnBackend, type Budget } from 'bolt402-ai-sdk';

export interface Bolt402ToolsConfig {
  backend: LnBackend;
  budget?: Budget;
  maxFeeSats?: number;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function createBolt402ToolsV6(config: Bolt402ToolsConfig): Record<string, any> {
  const client = new L402Client({
    backend: config.backend,
    tokenStore: new InMemoryTokenStore(),
    budget: config.budget,
    maxFeeSats: config.maxFeeSats,
  });

  const backend = config.backend;

  const l402FetchParams = z.object({
    url: z.string().describe('The URL to fetch'),
    method: z
      .enum(['GET', 'POST', 'PUT', 'DELETE'])
      .default('GET')
      .describe('HTTP method'),
    body: z
      .string()
      .optional()
      .describe('Request body for POST/PUT (JSON-encoded)'),
  });

  return {
    l402_fetch: (tool as any)({
      description:
        'Fetch a URL, automatically paying Lightning invoices for L402-gated APIs. ' +
        'When the server requires payment (HTTP 402), this tool pays the Lightning invoice, ' +
        'caches the token, and retries the request.',
      parameters: l402FetchParams,
      execute: async (args: z.infer<typeof l402FetchParams>) => {
        const response = await client.fetch(args.url, {
          method: args.method,
          body: args.body,
        });
        return {
          status: response.status,
          body: response.body,
          paid: response.paid,
          receipt: response.receipt
            ? {
                amountSats: response.receipt.amountSats,
                feeSats: response.receipt.feeSats,
                totalCostSats: response.receipt.totalCostSats,
                paymentHash: response.receipt.paymentHash,
                latencyMs: response.receipt.latencyMs,
              }
            : null,
        };
      },
    }),

    l402_get_balance: (tool as any)({
      description: 'Get the current Lightning node balance in satoshis.',
      parameters: z.object({}),
      execute: async () => {
        const balance = await backend.getBalance();
        const info = await backend.getInfo();
        return {
          balanceSats: balance,
          nodeAlias: info.alias,
          activeChannels: info.numActiveChannels,
        };
      },
    }),

    l402_get_receipts: (tool as any)({
      description: 'Get all L402 payment receipts from this session.',
      parameters: z.object({}),
      execute: async () => {
        const receipts = client.getReceipts();
        const totalSpent = client.getTotalSpent();
        return {
          totalSpentSats: totalSpent,
          paymentCount: receipts.length,
          receipts: receipts.map((r) => ({
            url: r.url,
            amountSats: r.amountSats,
            feeSats: r.feeSats,
            totalCostSats: r.totalCostSats,
            httpStatus: r.httpStatus,
            latencyMs: r.latencyMs,
            timestamp: r.timestamp,
          })),
        };
      },
    }),
  };
}
