import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { initSync } from "l402-wasm";

const __dirname = dirname(fileURLToPath(import.meta.url));
const wasmPath = resolve(__dirname, "node_modules/l402-wasm/l402_wasm_bg.wasm");
const wasmBytes = readFileSync(wasmPath);
initSync({ module: wasmBytes });
