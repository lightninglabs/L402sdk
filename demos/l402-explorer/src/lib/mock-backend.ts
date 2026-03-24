import { createHash, randomBytes } from 'crypto';

/**
 * A receipt from a mock L402 payment.
 *
 * Matches the WasmReceipt interface shape so it can be used interchangeably
 * with the real WASM client's receipts.
 */
interface MockReceipt {
  amountSats: number;
  feeSats: number;
  paymentHash: string;
  endpoint: string;
  responseStatus: number;
  timestamp: number;
  totalCostSats(): number;
}

/**
 * Response from a mock L402 request.
 *
 * Matches the WasmL402Response shape (status, paid, body, receipt).
 */
interface MockResponse {
  status: number;
  paid: boolean;
  body: string;
  receipt?: MockReceipt;
}

/**
 * Mock L402 client for demo purposes.
 *
 * Implements the same interface as WasmL402Client (get, post, totalSpent,
 * receipts) so it can be passed to createBolt402Tools. Makes real HTTP
 * requests and simulates L402 payments by generating fake preimage/hash
 * pairs when a 402 challenge is received.
 *
 * NOTE: The fake preimage won't satisfy real L402 servers that verify
 * SHA256(preimage) == payment_hash. This mock is intended for testing
 * the UI flow and works best against mock L402 servers.
 */
export class MockL402Client {
  private _receipts: MockReceipt[] = [];
  private _totalSpent = 0;

  async get(url: string): Promise<MockResponse> {
    return this.request(url, 'GET');
  }

  async post(url: string, body?: string): Promise<MockResponse> {
    return this.request(url, 'POST', body);
  }

  get totalSpent(): Promise<number> {
    return Promise.resolve(this._totalSpent);
  }

  async receipts(): Promise<MockReceipt[]> {
    return this._receipts;
  }

  private async request(
    url: string,
    method: string,
    body?: string,
  ): Promise<MockResponse> {
    const headers: Record<string, string> = {};
    if (body) headers['Content-Type'] = 'application/json';

    const response = await fetch(url, { method, body, headers });

    if (response.status !== 402) {
      return {
        status: response.status,
        paid: false,
        body: await response.text(),
      };
    }

    // Parse L402 challenge from WWW-Authenticate header
    const wwwAuth = response.headers.get('www-authenticate') || '';
    const macaroonMatch = wwwAuth.match(/macaroon="([^"]+)"/);
    const invoiceMatch = wwwAuth.match(/invoice="([^"]+)"/);

    if (!macaroonMatch || !invoiceMatch) {
      return {
        status: 402,
        paid: false,
        body: await response.text(),
      };
    }

    // Simulate payment — generate a valid preimage/hash pair
    await new Promise((r) => setTimeout(r, 300 + Math.random() * 400));
    const preimageBytes = randomBytes(32);
    const preimage = preimageBytes.toString('hex');
    const paymentHash = createHash('sha256')
      .update(preimageBytes)
      .digest('hex');
    const amountSats = 10 + Math.floor(Math.random() * 40);
    const feeSats = 1 + Math.floor(Math.random() * 3);

    // Retry with L402 token
    const retryResponse = await fetch(url, {
      method,
      body,
      headers: {
        ...headers,
        Authorization: `L402 ${macaroonMatch[1]}:${preimage}`,
      },
    });

    const receipt: MockReceipt = {
      amountSats,
      feeSats,
      paymentHash,
      endpoint: url,
      responseStatus: retryResponse.status,
      timestamp: Math.floor(Date.now() / 1000),
      totalCostSats() {
        return this.amountSats + this.feeSats;
      },
    };

    this._receipts.push(receipt);
    this._totalSpent += amountSats + feeSats;

    return {
      status: retryResponse.status,
      paid: true,
      body: await retryResponse.text(),
      receipt,
    };
  }
}
