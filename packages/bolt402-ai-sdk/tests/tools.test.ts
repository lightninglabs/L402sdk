import { describe, expect, it, vi } from 'vitest';
import { createBolt402Tools } from '../src/tools.js';

/**
 * Creates a mock WasmL402Client for testing.
 *
 * The mock simulates the WASM client's interface without requiring
 * actual WASM initialization or a Lightning node.
 */
function createMockWasmClient() {
  const receipts: any[] = [];

  return {
    get: vi.fn().mockImplementation(async (_url: string) => ({
      status: 200,
      paid: false,
      body: '{"data": "free"}',
      receipt: undefined,
    })),
    post: vi.fn().mockImplementation(async (_url: string, _body?: string) => ({
      status: 200,
      paid: false,
      body: '{"data": "free"}',
      receipt: undefined,
    })),
    get totalSpent() {
      return Promise.resolve(receipts.reduce((sum, r) => sum + r.amountSats + r.feeSats, 0));
    },
    receipts: vi.fn().mockImplementation(async () => receipts),
    _receipts: receipts,
  };
}

describe('createBolt402Tools', () => {
  it('returns expected tools', () => {
    const tools = createBolt402Tools({
      client: createMockWasmClient() as any,
    });

    expect(tools).toHaveProperty('l402_fetch');
    expect(tools).toHaveProperty('l402_get_receipts');
  });

  describe('l402_fetch', () => {
    it('fetches a free URL via GET', async () => {
      const mock = createMockWasmClient();
      const tools = createBolt402Tools({ client: mock as any });

      const result = await tools.l402_fetch.execute(
        { url: 'https://api.example.com/free', method: 'GET' },
        { toolCallId: 'test-1', messages: [] },
      );

      expect(result.status).toBe(200);
      expect(result.body).toBe('{"data": "free"}');
      expect(result.paid).toBe(false);
      expect(result.receipt).toBeNull();
      expect(mock.get).toHaveBeenCalledWith('https://api.example.com/free');
    });

    it('returns receipt when payment is made', async () => {
      const mock = createMockWasmClient();
      mock.get.mockResolvedValueOnce({
        status: 200,
        paid: true,
        body: '{"data": "premium"}',
        receipt: {
          amountSats: 50,
          feeSats: 1,
          totalCostSats: () => 51,
          paymentHash: 'abc123',
          endpoint: 'https://api.example.com/paid',
          timestamp: 1234567890,
          responseStatus: 200,
        },
      });

      const tools = createBolt402Tools({ client: mock as any });

      const result = await tools.l402_fetch.execute(
        { url: 'https://api.example.com/paid', method: 'GET' },
        { toolCallId: 'test-2', messages: [] },
      );

      expect(result.status).toBe(200);
      expect(result.paid).toBe(true);
      expect(result.receipt).not.toBeNull();
      expect(result.receipt!.amountSats).toBe(50);
      expect(result.receipt!.totalCostSats).toBe(51);
    });

    it('sends POST with body', async () => {
      const mock = createMockWasmClient();
      const tools = createBolt402Tools({ client: mock as any });

      await tools.l402_fetch.execute(
        {
          url: 'https://api.example.com/data',
          method: 'POST',
          body: '{"query": "test"}',
        },
        { toolCallId: 'test-3', messages: [] },
      );

      expect(mock.post).toHaveBeenCalledWith('https://api.example.com/data', '{"query": "test"}');
    });
  });

  describe('l402_get_receipts', () => {
    it('returns empty receipts initially', async () => {
      const mock = createMockWasmClient();
      const tools = createBolt402Tools({ client: mock as any });

      const result = await tools.l402_get_receipts.execute(
        {},
        { toolCallId: 'test-4', messages: [] },
      );

      expect(result.totalSpentSats).toBe(0);
      expect(result.paymentCount).toBe(0);
      expect(result.receipts).toEqual([]);
    });
  });
});
