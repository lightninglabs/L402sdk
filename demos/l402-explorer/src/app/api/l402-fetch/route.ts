import { NextRequest, NextResponse } from 'next/server';
import {
  L402Client,
  LndBackend,
  SwissKnifeBackend,
  type LnBackend,
} from 'bolt402-ai-sdk';
import { MockBackend } from '@/lib/mock-backend';

function createBackend(): LnBackend {
  const backendType = process.env.BACKEND_TYPE || 'mock';

  if (backendType === 'lnd' && process.env.LND_URL && process.env.LND_MACAROON) {
    return new LndBackend({
      url: process.env.LND_URL,
      macaroon: process.env.LND_MACAROON,
    });
  }

  if (
    backendType === 'swissknife' &&
    process.env.SWISSKNIFE_URL &&
    process.env.SWISSKNIFE_API_KEY
  ) {
    return new SwissKnifeBackend({
      url: process.env.SWISSKNIFE_URL,
      apiKey: process.env.SWISSKNIFE_API_KEY,
    });
  }

  return new MockBackend();
}

/**
 * API route that performs a full L402 flow server-side.
 *
 * Uses the same L402Client + configured backend as the AI chat,
 * so it can actually pay invoices (real or mock) and return data.
 */
export async function POST(req: NextRequest) {
  try {
    const { url, method = 'GET' } = await req.json();

    if (!url || typeof url !== 'string') {
      return NextResponse.json({ error: 'Missing url' }, { status: 400 });
    }

    const backend = createBackend();
    const client = new L402Client({
      backend,
      budget: { perRequestMax: 1000, dailyMax: 50000 },
      maxFeeSats: 100,
    });

    const startTime = Date.now();
    const response = await client.fetch(url, { method });
    const latencyMs = Date.now() - startTime;

    // Try to pretty-format JSON bodies
    let body = response.body;
    try {
      body = JSON.stringify(JSON.parse(body), null, 2);
    } catch {
      // Not JSON, keep as-is
    }

    const steps = [];
    steps.push({
      id: 'request',
      label: 'HTTP Request',
      status: 'complete',
      detail: `${method} ${url}`,
    });

    if (response.paid && response.receipt) {
      steps.push({
        id: 'challenge',
        label: '402 Payment Required',
        status: 'complete',
        detail: `Invoice paid: ${response.receipt.amountSats} sats + ${response.receipt.feeSats} sats fee`,
      });
      steps.push({
        id: 'payment',
        label: 'Lightning Payment',
        status: 'complete',
        detail: `Hash: ${response.receipt.paymentHash.substring(0, 20)}... (${response.receipt.latencyMs}ms)`,
      });
      steps.push({
        id: 'retry',
        label: 'Retry with Token',
        status: 'complete',
        detail: 'Authorization: L402 <macaroon>:<preimage>',
      });
      steps.push({
        id: 'response',
        label: 'Response Data',
        status: 'complete',
        detail: `Status ${response.status} — ${body.length} bytes`,
      });
    } else {
      steps.push({
        id: 'challenge',
        label: '402 Challenge',
        status: 'complete',
        detail: 'No payment required (non-402 response)',
      });
      steps.push({
        id: 'payment',
        label: 'Lightning Payment',
        status: 'complete',
        detail: 'Skipped — free endpoint',
      });
      steps.push({
        id: 'retry',
        label: 'Retry with Token',
        status: 'complete',
        detail: 'Skipped',
      });
      steps.push({
        id: 'response',
        label: 'Response Data',
        status: 'complete',
        detail: `Status ${response.status}`,
      });
    }

    return NextResponse.json({
      url,
      status: response.status,
      body,
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
      steps,
      latencyMs,
    });
  } catch (error) {
    let message = error instanceof Error ? error.message : 'Unknown error';

    // Improve common error messages
    if (message.includes('HTTP request to an HTTPS server')) {
      message += '. Check that LND_URL uses https:// (e.g. https://umbrel.local:8080)';
    }

    return NextResponse.json(
      {
        url: '',
        status: 0,
        body: '',
        paid: false,
        receipt: null,
        error: message,
        steps: [
          { id: 'request', label: 'HTTP Request', status: 'error', detail: message },
        ],
      },
      { status: 200 }, // Return 200 so frontend can display the error steps
    );
  }
}
