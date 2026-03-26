/**
 * WASM integration tests against the regtest environment.
 *
 * These tests verify the WASM bindings work end-to-end: constructing real
 * backends, making L402-gated requests, and paying real Lightning invoices
 * through the Aperture proxy.
 *
 * Skipped when the regtest environment is not running.
 *
 * Required env vars (from .env.regtest):
 *   L402_SERVER_URL, LND_REST_HOST, LND_MACAROON_HEX
 * Optional:
 *   SWISSKNIFE_API_URL, SWISSKNIFE_API_KEY
 */

import { describe, it, expect, beforeAll } from "vitest";
import { WasmL402Client, WasmBudgetConfig } from "bolt402-wasm";
import * as fs from "fs";
import * as path from "path";

// Load .env.regtest if available
function loadEnv() {
  const candidates = [
    path.resolve(__dirname, "../../../../tests/regtest/.env.regtest"),
    path.resolve(__dirname, "../../../../../tests/regtest/.env.regtest"),
  ];

  for (const p of candidates) {
    if (fs.existsSync(p)) {
      const content = fs.readFileSync(p, "utf-8");
      for (const line of content.split("\n")) {
        const trimmed = line.trim();
        if (!trimmed || trimmed.startsWith("#")) continue;
        const eqIdx = trimmed.indexOf("=");
        if (eqIdx === -1) continue;
        const key = trimmed.slice(0, eqIdx);
        const val = trimmed.slice(eqIdx + 1);
        if (!process.env[key]) {
          process.env[key] = val;
        }
      }
      return;
    }
  }
}

loadEnv();

const L402_SERVER_URL = process.env.L402_SERVER_URL || "http://localhost:8081";
const LND_REST_HOST = process.env.LND_REST_HOST || "";
const LND_MACAROON_HEX = process.env.LND_MACAROON_HEX || "";
const SWISSKNIFE_API_URL = process.env.SWISSKNIFE_API_URL || "";
const SWISSKNIFE_API_KEY = process.env.SWISSKNIFE_API_KEY || "";

const hasLnd = LND_REST_HOST !== "" && LND_MACAROON_HEX !== "";
const hasSwissKnife = SWISSKNIFE_API_URL !== "" && SWISSKNIFE_API_KEY !== "";

/**
 * Check if the regtest L402 server is reachable.
 */
async function isRegtestAvailable(): Promise<boolean> {
  try {
    const resp = await fetch(`${L402_SERVER_URL}/health`, {
      signal: AbortSignal.timeout(5000),
    });
    // Aperture returns 402 for protected routes; any response means it's up
    return resp.ok || resp.status === 402;
  } catch {
    return false;
  }
}

describe("Regtest: LND REST backend via WASM", () => {
  let available = false;

  beforeAll(async () => {
    available = hasLnd && (await isRegtestAvailable());
    if (!available) {
      console.log(
        "SKIP: regtest LND not available (set L402_SERVER_URL, LND_REST_HOST, LND_MACAROON_HEX)",
      );
    }
  });

  it("full L402 flow: GET → 402 → pay → 200", async () => {
    if (!available) return;

    const client = WasmL402Client.withLndRest(
      LND_REST_HOST,
      LND_MACAROON_HEX,
      WasmBudgetConfig.unlimited(),
      BigInt(100),
    );

    const response = await client.get(`${L402_SERVER_URL}/api/data`);
    expect(response.status).toBe(200);
    expect(response.paid).toBe(true);
    expect(response.cachedToken).toBe(false);

    const receipt = response.receipt;
    expect(receipt).toBeDefined();
    expect(receipt!.amountSats).toBe(BigInt(100));
    expect(receipt!.responseStatus).toBe(200);
    expect(receipt!.paymentHash).toBeTruthy();
    expect(receipt!.preimage).toBeTruthy();

    const body = JSON.parse(response.body);
    expect(body.ok).toBe(true);
    expect(body.resource).toBe("data");
  });

  it("token caching: second request skips payment", async () => {
    if (!available) return;

    const client = WasmL402Client.withLndRest(
      LND_REST_HOST,
      LND_MACAROON_HEX,
      WasmBudgetConfig.unlimited(),
      BigInt(100),
    );

    // First request pays
    const resp1 = await client.get(`${L402_SERVER_URL}/api/cheap`);
    expect(resp1.paid).toBe(true);

    // Second request uses cache
    const resp2 = await client.get(`${L402_SERVER_URL}/api/cheap`);
    expect(resp2.paid).toBe(false);
    expect(resp2.cachedToken).toBe(true);
    expect(resp2.status).toBe(200);
  });

  it("non-existent endpoint returns 404 without payment", async () => {
    if (!available) return;

    const client = WasmL402Client.withLndRest(
      LND_REST_HOST,
      LND_MACAROON_HEX,
      WasmBudgetConfig.unlimited(),
      BigInt(100),
    );

    const response = await client.get(`${L402_SERVER_URL}/api/nonexistent`);
    expect(response.status).toBe(404);
    expect(response.paid).toBe(false);
  });

  it("tracks total spent", async () => {
    if (!available) return;

    const client = WasmL402Client.withLndRest(
      LND_REST_HOST,
      LND_MACAROON_HEX,
      WasmBudgetConfig.unlimited(),
      BigInt(100),
    );

    await client.get(`${L402_SERVER_URL}/api/cheap`);
    const spent = await client.totalSpent();
    expect(Number(spent)).toBe(10);
  });
});

describe("Regtest: SwissKnife backend via WASM", () => {
  let available = false;

  beforeAll(async () => {
    available = hasSwissKnife && (await isRegtestAvailable());
    if (!available) {
      console.log(
        "SKIP: regtest SwissKnife not available (set SWISSKNIFE_API_URL, SWISSKNIFE_API_KEY)",
      );
    }
  });

  it("full L402 flow via SwissKnife", async () => {
    if (!available) return;

    const client = WasmL402Client.withSwissKnife(
      SWISSKNIFE_API_URL,
      SWISSKNIFE_API_KEY,
      WasmBudgetConfig.unlimited(),
      BigInt(100),
    );

    const response = await client.get(`${L402_SERVER_URL}/api/data`);
    expect(response.status).toBe(200);
    expect(response.paid).toBe(true);

    const body = JSON.parse(response.body);
    expect(body.ok).toBe(true);
  });
});
