import { describe, it, expect } from "vitest";
import { WasmBudgetConfig, WasmL402Client } from "bolt402-wasm";

describe("WasmBudgetConfig", () => {
  it("constructs with all parameters", () => {
    const budget = new WasmBudgetConfig(
      BigInt(1000),
      BigInt(5000),
      BigInt(50000),
      BigInt(1000000),
    );
    expect(budget).toBeDefined();
    expect(budget.perRequestMax).toBe(BigInt(1000));
    expect(budget.hourlyMax).toBe(BigInt(5000));
    expect(budget.dailyMax).toBe(BigInt(50000));
    expect(budget.totalMax).toBe(BigInt(1000000));
  });

  it("creates unlimited budget", () => {
    const budget = WasmBudgetConfig.unlimited();
    expect(budget).toBeDefined();
    expect(budget.perRequestMax).toBe(BigInt(0));
    expect(budget.hourlyMax).toBe(BigInt(0));
    expect(budget.dailyMax).toBe(BigInt(0));
    expect(budget.totalMax).toBe(BigInt(0));
  });

  it("treats 0 as no limit", () => {
    const budget = new WasmBudgetConfig(
      BigInt(0),
      BigInt(100),
      BigInt(0),
      BigInt(500),
    );
    expect(budget.perRequestMax).toBe(BigInt(0));
    expect(budget.hourlyMax).toBe(BigInt(100));
    expect(budget.dailyMax).toBe(BigInt(0));
    expect(budget.totalMax).toBe(BigInt(500));
  });
});

describe("WasmL402Client", () => {
  it("constructs via withLndRest", () => {
    const client = WasmL402Client.withLndRest(
      "https://localhost:8080",
      "deadbeefcafebabe",
      WasmBudgetConfig.unlimited(),
      BigInt(100),
    );
    expect(client).toBeDefined();
  });

  it("exposes get method", () => {
    const client = WasmL402Client.withLndRest(
      "https://localhost:8080",
      "deadbeefcafebabe",
      WasmBudgetConfig.unlimited(),
      BigInt(100),
    );
    expect(typeof client.get).toBe("function");
  });

  it("exposes post method", () => {
    const client = WasmL402Client.withLndRest(
      "https://localhost:8080",
      "deadbeefcafebabe",
      WasmBudgetConfig.unlimited(),
      BigInt(100),
    );
    expect(typeof client.post).toBe("function");
  });

  it("exposes receipts method", () => {
    const client = WasmL402Client.withLndRest(
      "https://localhost:8080",
      "deadbeefcafebabe",
      WasmBudgetConfig.unlimited(),
      BigInt(100),
    );
    expect(typeof client.receipts).toBe("function");
  });

  it("constructs via withSwissKnife", () => {
    const client = WasmL402Client.withSwissKnife(
      "https://api.numeraire.tech",
      "sk-test-key",
      WasmBudgetConfig.unlimited(),
      BigInt(100),
    );
    expect(client).toBeDefined();
  });
});
