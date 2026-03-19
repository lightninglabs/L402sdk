/** A service from 402index.io. */
export interface L402Service {
  id: string;
  name: string;
  description: string;
  url: string;
  protocol: string;
  price_sats: number | null;
  price_usd: number | null;
  payment_asset: string | null;
  payment_network: string | null;
  category: string;
  provider: string;
  source: string;
  featured: number;
  health_status: 'healthy' | 'degraded' | 'down' | 'unknown';
  uptime_30d: number | null;
  latency_p50_ms: number | null;
  last_checked: string | null;
  registered_at: string;
  http_method: string;
  reliability_score: number | null;
}

/** Response from 402index.io API. */
export interface IndexResponse {
  services: L402Service[];
  total: number;
  limit: number;
  offset: number;
}

/** The step-by-step protocol flow for display. */
export interface ProtocolStep {
  id: string;
  label: string;
  description: string;
  status: 'pending' | 'active' | 'complete' | 'error';
  detail?: string;
}

/** Result of an L402 fetch operation. */
export interface FetchResult {
  url: string;
  status: number;
  body: string;
  paid: boolean;
  receipt: {
    amountSats: number;
    feeSats: number;
    totalCostSats: number;
    paymentHash: string;
    latencyMs: number;
  } | null;
  error?: string;
}

/** A receipt for the spending dashboard. */
export interface SpendingEntry {
  url: string;
  service: string;
  amountSats: number;
  feeSats: number;
  timestamp: string;
  status: number;
  latencyMs: number;
}

/** Category filter option. */
export interface CategoryOption {
  slug: string;
  name: string;
  count: number;
}
