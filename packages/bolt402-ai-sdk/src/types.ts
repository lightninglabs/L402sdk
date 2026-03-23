/**
 * Core type definitions for bolt402-ai-sdk.
 *
 * These types mirror the Rust bolt402-core port definitions,
 * providing the same hexagonal architecture in TypeScript.
 */

/** Result of a successful Lightning payment. */
export interface PaymentResult {
  /** Hex-encoded payment preimage (proof of payment). */
  preimage: string;
  /** Hex-encoded payment hash. */
  paymentHash: string;
  /** Amount paid in satoshis (excluding fees). */
  amountSats: number;
  /** Routing fee paid in satoshis. */
  feeSats: number;
}

/** Information about a Lightning node. */
export interface NodeInfo {
  /** Node public key (hex-encoded). */
  pubkey: string;
  /** Node alias. */
  alias: string;
  /** Number of active channels. */
  numActiveChannels: number;
}

/**
 * Lightning Network backend port.
 *
 * Implementations provide the ability to pay invoices and query node state.
 * Each backend (LND, SwissKnife, etc.) provides an implementation.
 */
export interface LnBackend {
  /** Pay a BOLT11 Lightning invoice. */
  payInvoice(bolt11: string, maxFeeSats: number): Promise<PaymentResult>;
  /** Get the current spendable balance in satoshis. */
  getBalance(): Promise<number>;
  /** Get information about the connected Lightning node. */
  getInfo(): Promise<NodeInfo>;
}

/** A cached L402 token (macaroon + preimage pair). */
export interface CachedToken {
  macaroon: string;
  preimage: string;
}

/**
 * Token storage port.
 *
 * Implementations cache L402 tokens to avoid re-paying for the same resource.
 */
export interface TokenStore {
  /** Retrieve a cached token for an endpoint, if one exists. */
  get(endpoint: string): Promise<CachedToken | null>;
  /** Store a token for a given endpoint. */
  put(endpoint: string, macaroon: string, preimage: string): Promise<void>;
  /** Remove a cached token for an endpoint. */
  remove(endpoint: string): Promise<void>;
  /** Clear all cached tokens. */
  clear(): Promise<void>;
}

/** Budget configuration for L402 payments. */
export interface Budget {
  /** Maximum satoshis per single request. */
  perRequestMax?: number;
  /** Maximum satoshis per hour. */
  hourlyMax?: number;
  /** Maximum satoshis per day (24h rolling). */
  dailyMax?: number;
  /** Maximum total satoshis across all payments. */
  totalMax?: number;
}

/** A payment receipt for audit and cost analysis. */
export interface Receipt {
  /** The URL that was paid for. */
  url: string;
  /** Amount paid in satoshis (excluding fees). */
  amountSats: number;
  /** Routing fee in satoshis. */
  feeSats: number;
  /** Total cost (amount + fee). */
  totalCostSats: number;
  /** Hex-encoded payment hash. */
  paymentHash: string;
  /** Hex-encoded payment preimage. */
  preimage: string;
  /** HTTP status code of the response after payment. */
  httpStatus: number;
  /** Latency in milliseconds (including payment time). */
  latencyMs: number;
  /** ISO-8601 timestamp of when the payment was made. */
  timestamp: string;
}

/** Parsed L402 challenge from a WWW-Authenticate header. */
export interface L402Challenge {
  /** Base64-encoded macaroon. */
  macaroon: string;
  /** BOLT11 invoice string. */
  invoice: string;
}

/** Configuration for the L402Client. */
export interface L402ClientConfig {
  /** Lightning backend for paying invoices. */
  backend: LnBackend;
  /** Token store for caching L402 credentials. Optional, defaults to in-memory. */
  tokenStore?: TokenStore;
  /** Budget limits. Optional, defaults to unlimited. */
  budget?: Budget;
  /** Maximum routing fee in satoshis. Default: 100. */
  maxFeeSats?: number;
  /** Custom fetch function (for testing or custom HTTP handling). */
  fetchFn?: typeof fetch;
}

/** Response from an L402-aware HTTP request. */
export interface L402Response {
  /** HTTP status code. */
  status: number;
  /** Response headers. */
  headers: Record<string, string>;
  /** Response body as string. */
  body: string;
  /** Whether a Lightning payment was made for this request. */
  paid: boolean;
  /** Payment receipt, if a payment was made. */
  receipt: Receipt | null;
  /** Whether a previously cached L402 token was used (no new payment needed). */
  cachedToken: boolean;
}
