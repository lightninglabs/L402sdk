import { describe, it, expect } from "vitest";
import { setPanicHook } from "l402-wasm";

describe("WASM initialization", () => {
  it("loads the WASM module successfully via initSync", () => {
    // If we reach this point, setup.ts already called initSync successfully.
    // Verify by calling a WASM function.
    expect(() => setPanicHook()).not.toThrow();
  });

  it("setPanicHook is idempotent", () => {
    // Calling it multiple times should not throw.
    setPanicHook();
    setPanicHook();
    setPanicHook();
  });
});
