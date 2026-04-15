/** Code snippets for the L402sdk vs lnget comparison. */

export interface Snippet {
  language: string;
  label: string;
  code: string;
  /** Shiki language identifier. */
  shikiLang: string;
}

export const lngetSnippets: Snippet[] = [
  {
    language: "bash",
    label: "Shell",
    shikiLang: "bash",
    code: `# Fetch weather data from an L402-gated API
$ lnget -q https://api.example.com/v1/weather?city=zurich | jq .

# In a script: subprocess, string parsing, no types
result=$(lnget -q --json https://api.example.com/v1/weather?city=zurich)
status=$(echo "$result" | jq -r '.status')
cost=$(echo "$result" | jq -r '.payment.amount_sat')

# Budget control via CLI flags
$ lnget --max-cost 5000 --max-fee 50 https://api.example.com/data

# Token management via separate commands
$ lnget tokens list
$ lnget tokens remove api.example.com`,
  },
  {
    language: "python",
    label: "Python (subprocess)",
    shikiLang: "python",
    code: `import subprocess
import json

# Shell out to lnget — no native integration
result = subprocess.run(
    ["lnget", "-q", "--json",
     "https://api.example.com/v1/weather?city=zurich"],
    capture_output=True, text=True
)

if result.returncode != 0:
    raise RuntimeError(f"lnget failed: {result.stderr}")

data = json.loads(result.stdout)
weather = data  # raw dict, no type safety
cost = data.get("payment", {}).get("amount_sat", 0)`,
  },
  {
    language: "typescript",
    label: "TypeScript (exec)",
    shikiLang: "typescript",
    code: `import { execSync } from "child_process";

// Shell out to lnget — no native types, no error typing
const raw = execSync(
  'lnget -q --json "https://api.example.com/v1/weather?city=zurich"',
  { encoding: "utf-8" }
);

const data = JSON.parse(raw);       // any — no type safety
const cost = data.payment?.amount_sat ?? 0;
const weather = data;                // raw object`,
  },
];

export const L402sdkSnippets: Snippet[] = [
  {
    language: "rust",
    label: "Rust",
    shikiLang: "rust",
    code: `use l402_core::{L402Client, Budget};
use l402_lnd::LndBackend;

let client = L402Client::builder()
    .ln_backend(LndBackend::new(&config))
    .budget(Budget::per_request(1000))
    .build()?;

// Typed response, automatic 402 handling, token caching
let response = client
    .get("https://api.example.com/v1/weather?city=zurich")
    .await?;
let weather: Weather = response.json().await?;

// Budget and receipts are accessible programmatically
let spent = client.total_spent().await;
let receipts = client.receipts().await;`,
  },
  {
    language: "typescript",
    label: "TypeScript (Vercel AI SDK)",
    shikiLang: "typescript",
    code: `import { createL402Tools, LndBackend } from "l402-ai-sdk";
import { generateText } from "ai";
import { openai } from "@ai-sdk/openai";

const tools = createL402Tools({
  backend: new LndBackend({
    url: process.env.LND_URL!,
    macaroon: process.env.LND_MACAROON!,
  }),
  budget: { perRequestMax: 1000, dailyMax: 50_000 },
});

// AI agent autonomously pays for L402 APIs
const { text } = await generateText({
  model: openai("gpt-4o"),
  tools,
  prompt: "Get the weather for Zurich",
});`,
  },
  {
    language: "python",
    label: "Python",
    shikiLang: "python",
    code: `from l402 import L402Client, LndBackend, Budget

client = L402Client(
    backend=LndBackend(url=url, macaroon=macaroon),
    budget=Budget(per_request_max=1000),
)

# Typed response, automatic 402 handling, token caching
response = client.get(
    "https://api.example.com/v1/weather?city=zurich"
)
weather = response.json()  # dict with typed wrapper

# Budget tracking built in
spent = client.total_spent()
receipts = client.receipts()`,
  },
];

export interface FeatureRow {
  feature: string;
  lnget: string;
  l402: string;
  advantage: "L402sdk" | "lnget" | "neutral";
}

export const featureComparison: FeatureRow[] = [
  {
    feature: "Integration model",
    lnget: "CLI binary — subprocess/exec",
    l402: "Library — native import",
    advantage: "L402sdk",
  },
  {
    feature: "Error handling",
    lnget: "Exit codes + stderr strings",
    l402: "Typed Result<T, E> / exceptions",
    advantage: "L402sdk",
  },
  {
    feature: "Token caching",
    lnget: "File-based (~/.lnget/tokens)",
    l402: "Pluggable (memory, file, localStorage, custom)",
    advantage: "L402sdk",
  },
  {
    feature: "Budget control",
    lnget: "--max-cost / --max-fee flags",
    l402: "Programmatic (per-request, hourly, daily, per-domain)",
    advantage: "L402sdk",
  },
  {
    feature: "AI framework support",
    lnget: "Shell out from agent code",
    l402: "Native Vercel AI SDK tools, LangChain, etc.",
    advantage: "L402sdk",
  },
  {
    feature: "Languages",
    lnget: "Go (source) — consume via shell",
    l402: "Rust, TypeScript, Python (+ Go, WASM planned)",
    advantage: "L402sdk",
  },
  {
    feature: "Response types",
    lnget: "Raw JSON string",
    l402: "Typed structs / interfaces",
    advantage: "L402sdk",
  },
  {
    feature: "Receipt tracking",
    lnget: "JSON output per-request",
    l402: "Built-in receipt log with programmatic access",
    advantage: "L402sdk",
  },
  {
    feature: "Streaming support",
    lnget: "stdout pipe",
    l402: "Native async streams",
    advantage: "L402sdk",
  },
  {
    feature: "Quick CLI usage",
    lnget: "One-liner: lnget <url>",
    l402: "Requires code — not a CLI tool",
    advantage: "lnget",
  },
];
