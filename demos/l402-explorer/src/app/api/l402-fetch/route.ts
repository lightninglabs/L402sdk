import { NextRequest, NextResponse } from 'next/server';
import { getSharedL402Client } from '@/lib/l402-shared';

/**
 * API route that performs a full L402 flow server-side.
 *
 * Uses the shared L402 client so receipts accumulate across all routes
 * and can be queried via /api/l402-receipts.
 */
export async function POST(req: NextRequest) {
  try {
    const { url, method = 'GET' } = await req.json();

    if (!url || typeof url !== 'string') {
      return NextResponse.json({ error: 'Missing url' }, { status: 400 });
    }

    const client = getSharedL402Client();

    const startTime = Date.now();
    const response =
      method === 'POST'
        ? await client.post(url)
        : await client.get(url);
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
        detail: `Hash: ${response.receipt.paymentHash.substring(0, 20)}... (${Number(response.receipt.latencyMs)}ms)`,
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
    } else if (response.cachedToken) {
      steps.push({
        id: 'challenge',
        label: 'Cached L402 Token',
        status: 'complete',
        detail: 'Used previously paid token — no new payment needed',
      });
      steps.push({
        id: 'payment',
        label: 'Lightning Payment',
        status: 'complete',
        detail: 'Skipped — token still valid',
      });
      steps.push({
        id: 'retry',
        label: 'Authenticated Request',
        status: 'complete',
        detail: 'Authorization: L402 <macaroon>:<preimage> (cached)',
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
            amountSats: Number(response.receipt.amountSats),
            feeSats: Number(response.receipt.feeSats),
            totalCostSats: Number(response.receipt.totalCostSats()),
            paymentHash: response.receipt.paymentHash,
            latencyMs: Number(response.receipt.latencyMs),
          }
        : null,
      steps,
      latencyMs,
    });
  } catch (error) {
    // WASM errors may be strings or JsValues, not Error instances
    let message = error instanceof Error ? error.message : String(error);
    const cause = error instanceof Error ? (error.cause as Error)?.message ?? '' : '';

    // Improve common error messages
    if (message === 'fetch failed' || message === 'Failed to fetch') {
      if (cause.includes('self-signed certificate') || cause.includes('certificate')) {
        message = `TLS error: the service uses a self-signed certificate. Set NODE_TLS_REJECT_UNAUTHORIZED=0 in .env.local for local development.`;
      } else if (cause) {
        message = `Could not reach service: ${cause}`;
      } else {
        message = 'Could not reach service (DNS resolution failed, connection refused, or TLS error)';
      }
    } else if (message.includes('HTTP request to an HTTPS server')) {
      message += '. Check that LND_URL uses https:// (e.g. https://umbrel.local:8080)';
    } else if (message.includes('ECONNREFUSED')) {
      message = `Connection refused. The service may be down.`;
    } else if (message.includes('ENOTFOUND')) {
      message = `DNS lookup failed: the hostname could not be resolved. The service may no longer exist.`;
    } else if (message.includes('certificate')) {
      message = `TLS/SSL error: ${message}. The service may have an invalid certificate.`;
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
