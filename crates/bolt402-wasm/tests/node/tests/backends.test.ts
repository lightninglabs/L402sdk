import { describe, it, expect } from "vitest";
import {
  WasmLndRestBackend,
  WasmSwissKnifeBackend,
  WasmClnRestBackend,
  WasmNodeInfo,
  WasmPaymentResult,
} from "bolt402-wasm";

describe("WasmLndRestBackend", () => {
  it("constructs with url and macaroon", () => {
    const backend = new WasmLndRestBackend(
      "https://localhost:8080",
      "deadbeefcafebabe",
    );
    expect(backend).toBeDefined();
  });

  it("exposes payInvoice method", () => {
    const backend = new WasmLndRestBackend(
      "https://localhost:8080",
      "deadbeefcafebabe",
    );
    expect(typeof backend.payInvoice).toBe("function");
  });

  it("exposes getBalance method", () => {
    const backend = new WasmLndRestBackend(
      "https://localhost:8080",
      "deadbeefcafebabe",
    );
    expect(typeof backend.getBalance).toBe("function");
  });

  it("exposes getInfo method", () => {
    const backend = new WasmLndRestBackend(
      "https://localhost:8080",
      "deadbeefcafebabe",
    );
    expect(typeof backend.getInfo).toBe("function");
  });
});

describe("WasmSwissKnifeBackend", () => {
  it("constructs with url and api_key", () => {
    const backend = new WasmSwissKnifeBackend(
      "https://api.numeraire.tech",
      "sk-test-key",
    );
    expect(backend).toBeDefined();
  });

  it("exposes payInvoice method", () => {
    const backend = new WasmSwissKnifeBackend(
      "https://api.numeraire.tech",
      "sk-test-key",
    );
    expect(typeof backend.payInvoice).toBe("function");
  });

  it("exposes getBalance method", () => {
    const backend = new WasmSwissKnifeBackend(
      "https://api.numeraire.tech",
      "sk-test-key",
    );
    expect(typeof backend.getBalance).toBe("function");
  });

  it("exposes getInfo method", () => {
    const backend = new WasmSwissKnifeBackend(
      "https://api.numeraire.tech",
      "sk-test-key",
    );
    expect(typeof backend.getInfo).toBe("function");
  });
});

describe("WasmClnRestBackend", () => {
  it("constructs with url and rune", () => {
    const backend = new WasmClnRestBackend(
      "https://localhost:3001",
      "rune-token-value",
    );
    expect(backend).toBeDefined();
  });

  it("constructs with withRune static method", () => {
    const backend = WasmClnRestBackend.withRune(
      "https://localhost:3001",
      "rune-token-value",
    );
    expect(backend).toBeDefined();
  });

  it("exposes payInvoice method", () => {
    const backend = new WasmClnRestBackend(
      "https://localhost:3001",
      "rune-token-value",
    );
    expect(typeof backend.payInvoice).toBe("function");
  });

  it("exposes getBalance method", () => {
    const backend = new WasmClnRestBackend(
      "https://localhost:3001",
      "rune-token-value",
    );
    expect(typeof backend.getBalance).toBe("function");
  });

  it("exposes getInfo method", () => {
    const backend = new WasmClnRestBackend(
      "https://localhost:3001",
      "rune-token-value",
    );
    expect(typeof backend.getInfo).toBe("function");
  });
});

describe("Type exports", () => {
  it("WasmNodeInfo is importable", () => {
    expect(WasmNodeInfo).toBeDefined();
  });

  it("WasmPaymentResult is importable", () => {
    expect(WasmPaymentResult).toBeDefined();
  });
});
