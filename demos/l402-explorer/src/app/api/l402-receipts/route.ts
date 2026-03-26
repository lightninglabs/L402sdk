import { NextResponse } from 'next/server';
import { getSharedL402Client } from '@/lib/l402-shared';

/**
 * Returns all L402 payment receipts from the shared client.
 *
 * The spending dashboard polls this endpoint to stay in sync
 * with payments made via the chat agent or the l402-fetch route.
 */
export async function GET() {
  const client = getSharedL402Client();
  const receipts = await client.receipts();
  const totalSpent = await client.totalSpent();

  const receiptList = Array.isArray(receipts) ? receipts : [];

  return NextResponse.json({
    totalSpentSats: Number(totalSpent),
    paymentCount: receiptList.length,
    receipts: receiptList.map((r: any) => ({
      url: r.endpoint,
      amountSats: Number(r.amountSats),
      feeSats: Number(r.feeSats),
      totalCostSats: Number(r.totalCostSats()),
      paymentHash: r.paymentHash,
      httpStatus: r.responseStatus,
      latencyMs: Number(r.latencyMs),
      timestamp: Number(r.timestamp),
    })),
  });
}
