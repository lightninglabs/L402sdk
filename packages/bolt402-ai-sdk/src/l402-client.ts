/**
 * L402 client engine.
 *
 * The L402Client handles the full L402 protocol flow: making HTTP requests,
 * detecting 402 challenges, paying invoices, caching tokens, enforcing
 * budgets, and recording receipts.
 *
 * This is a TypeScript port of the Rust bolt402-core L402Client, following
 * the same hexagonal architecture with pluggable backends and token stores.
 */

import { BudgetTracker } from './budget.js';
import { InMemoryTokenStore } from './token-store.js';
import type {
  L402Challenge,
  L402ClientConfig,
  L402Response,
  LnBackend,
  Receipt,
  TokenStore,
} from './types.js';

/** Parse an L402 challenge from a WWW-Authenticate header value. */
export function parseL402Challenge(header: string): L402Challenge | null {
  // Format: L402 macaroon="<base64>", invoice="<bolt11>"
  // Also support: L402 <macaroon>:<invoice> (compact format)
  const trimmed = header.trim();

  // Standard format with quoted parameters
  const macaroonMatch = /macaroon="([^"]+)"/.exec(trimmed);
  const invoiceMatch = /invoice="([^"]+)"/.exec(trimmed);

  if (macaroonMatch && invoiceMatch) {
    return {
      macaroon: macaroonMatch[1],
      invoice: invoiceMatch[1],
    };
  }

  // Compact format: L402 <macaroon>:<invoice>
  if (trimmed.startsWith('L402 ') || trimmed.startsWith('LSAT ')) {
    const payload = trimmed.slice(trimmed.indexOf(' ') + 1);
    const colonIdx = payload.indexOf(':');
    if (colonIdx > 0) {
      return {
        macaroon: payload.slice(0, colonIdx),
        invoice: payload.slice(colonIdx + 1),
      };
    }
  }

  return null;
}

/** Construct the L402 Authorization header value. */
function toAuthHeader(macaroon: string, preimage: string): string {
  return `L402 ${macaroon}:${preimage}`;
}

/**
 * L402 client that handles the full payment-gated HTTP flow.
 *
 * Intercepts HTTP 402 responses, parses L402 challenges, pays Lightning
 * invoices, and retries requests with valid credentials.
 */
export class L402Client {
  private readonly backend: LnBackend;
  private readonly tokenStore: TokenStore;
  private readonly budgetTracker: BudgetTracker;
  private readonly maxFeeSats: number;
  private readonly fetchFn: typeof fetch;
  private readonly receipts: Receipt[] = [];

  constructor(config: L402ClientConfig) {
    this.backend = config.backend;
    this.tokenStore = config.tokenStore ?? new InMemoryTokenStore();
    this.budgetTracker = new BudgetTracker(config.budget);
    this.maxFeeSats = config.maxFeeSats ?? 100;
    this.fetchFn = config.fetchFn ?? fetch;
  }

  /**
   * Fetch a URL, automatically handling L402 payment challenges.
   *
   * If the server responds with HTTP 402, the client will:
   * 1. Parse the L402 challenge from the WWW-Authenticate header
   * 2. Check the budget to ensure the payment is allowed
   * 3. Pay the Lightning invoice
   * 4. Cache the resulting token
   * 5. Retry the request with the Authorization: L402 header
   */
  async fetch(
    url: string,
    options: {
      method?: string;
      body?: string;
      headers?: Record<string, string>;
    } = {},
  ): Promise<L402Response> {
    const method = options.method ?? 'GET';
    const startTime = Date.now();

    // Check if we have a cached token for this endpoint
    const cached = await this.tokenStore.get(url);
    if (cached) {
      const response = await this.sendWithAuth(url, method, options.body, options.headers, cached.macaroon, cached.preimage);

      if (response.status !== 402) {
        return this.buildResponse(response, false, null, true);
      }

      // Token was rejected, remove from cache
      await this.tokenStore.remove(url);
    }

    // Make the initial request without auth
    const response = await this.sendRequest(url, method, options.body, options.headers);

    // If not 402, return as-is
    if (response.status !== 402) {
      return this.buildResponse(response, false, null);
    }

    // Parse the L402 challenge
    const wwwAuth = response.headers.get('www-authenticate');
    if (!wwwAuth) {
      throw new L402Error('Server returned 402 but no WWW-Authenticate header');
    }

    const challenge = parseL402Challenge(wwwAuth);
    if (!challenge) {
      throw new L402Error(`Failed to parse L402 challenge from header: ${wwwAuth}`);
    }

    // Pay the invoice
    const payment = await this.backend.payInvoice(challenge.invoice, this.maxFeeSats);

    // Check budget (using actual amount from payment result)
    this.budgetTracker.checkAndRecord(payment.amountSats + payment.feeSats);

    // Cache the token
    await this.tokenStore.put(url, challenge.macaroon, payment.preimage);

    // Retry with auth
    const retryResponse = await this.sendWithAuth(
      url,
      method,
      options.body,
      options.headers,
      challenge.macaroon,
      payment.preimage,
    );

    if (retryResponse.status === 402) {
      await this.tokenStore.remove(url);
      const retryBody = await retryResponse.clone().text().catch(() => '');
      throw new L402Error(`Server returned 402 again after payment. Response: ${retryBody.slice(0, 300)}`);
    }

    const latencyMs = Date.now() - startTime;

    const receipt: Receipt = {
      url,
      amountSats: payment.amountSats,
      feeSats: payment.feeSats,
      totalCostSats: payment.amountSats + payment.feeSats,
      paymentHash: payment.paymentHash,
      preimage: payment.preimage,
      httpStatus: retryResponse.status,
      latencyMs,
      timestamp: new Date().toISOString(),
    };

    this.receipts.push(receipt);

    return this.buildResponse(retryResponse, true, receipt);
  }

  /** Send a GET request with L402 handling. */
  async get(url: string, headers?: Record<string, string>): Promise<L402Response> {
    return this.fetch(url, { method: 'GET', headers });
  }

  /** Send a POST request with L402 handling. */
  async post(url: string, body?: string, headers?: Record<string, string>): Promise<L402Response> {
    return this.fetch(url, { method: 'POST', body, headers });
  }

  /** Get all recorded payment receipts. */
  getReceipts(): Receipt[] {
    return [...this.receipts];
  }

  /** Get the total amount spent in satoshis. */
  getTotalSpent(): number {
    return this.budgetTracker.getTotalSpent();
  }

  /** Get the Lightning backend (for direct access to balance/info). */
  getBackend(): LnBackend {
    return this.backend;
  }

  private async sendRequest(
    url: string,
    method: string,
    body?: string,
    headers?: Record<string, string>,
  ): Promise<Response> {
    const requestHeaders: Record<string, string> = {
      'User-Agent': 'bolt402-ai-sdk/0.1.0',
      ...headers,
    };

    if (body) {
      requestHeaders['Content-Type'] = requestHeaders['Content-Type'] ?? 'application/json';
    }

    return this.fetchFn(url, {
      method,
      headers: requestHeaders,
      body: body ?? undefined,
    });
  }

  private async sendWithAuth(
    url: string,
    method: string,
    body?: string,
    headers?: Record<string, string>,
    macaroon?: string,
    preimage?: string,
  ): Promise<Response> {
    const authHeader = toAuthHeader(macaroon ?? '', preimage ?? '');

    return this.sendRequest(url, method, body, {
      ...headers,
      Authorization: authHeader,
    });
  }

  private async buildResponse(
    response: Response,
    paid: boolean,
    receipt: Receipt | null,
    cachedToken = false,
  ): Promise<L402Response> {
    const headers: Record<string, string> = {};
    response.headers.forEach((value, key) => {
      headers[key] = value;
    });

    const bodyText = await response.text();

    return {
      status: response.status,
      headers,
      body: bodyText,
      paid,
      receipt,
      cachedToken,
    };
  }
}

/** Error type for L402 protocol failures. */
export class L402Error extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'L402Error';
  }
}
