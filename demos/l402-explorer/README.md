# L402 Explorer

Interactive demo showcasing [bolt402](https://github.com/lightninglabs/bolt402). Browse L402-gated APIs, then ask an AI agent to query them — bolt402 handles the Lightning payments automatically.

## What it does

- **Service Browser**: Browse real L402 services from [satring.com](https://satring.com) with search and category filters
- **AI Research Assistant**: Chat panel powered by Vercel AI SDK. Ask a question, the agent identifies which L402 APIs to call, pays the Lightning invoice via bolt402, and presents the data with cost attribution
- **Spending Dashboard**: Tracks every payment — service name, cost in sats, latency, payment hash

The AI agent uses `createBolt402Tools()` from bolt402-ai-sdk, which gives it:
- `l402_fetch` — fetch any URL, automatically handling 402 challenges and Lightning payments
- `l402_get_balance` — check the Lightning node balance
- `l402_get_receipts` — audit trail of all payments

## Prerequisites

- Node.js 20+
- [Corepack](https://yarnpkg.com/corepack) enabled (`corepack enable`)
- An AI provider API key (Anthropic, xAI, or OpenAI)
- A Lightning backend (LND or SwissKnife)

## Quick start

```bash
# Install dependencies
yarn install

# Copy and configure environment variables
cp .env.example .env.local
# Edit .env.local — set an AI provider API key and a Lightning backend.

# Run in development mode
yarn dev
```

Open [http://localhost:3000](http://localhost:3000).

## Environment variables

Create `.env.local` from `.env.example`:

Pick ONE AI provider (auto-detected, priority: Anthropic > xAI > OpenAI):

```bash
ANTHROPIC_API_KEY=sk-ant-...   # Claude
# XAI_API_KEY=xai-...          # Grok
# OPENAI_API_KEY=sk-...        # GPT
# AI_MODEL=claude-sonnet-4-20250514  # optional model override
```

### Connecting LND

If you have an LND node (e.g. on Umbrel):

```bash
BACKEND_TYPE=lnd
LND_URL=https://your-umbrel-ip:8080
LND_MACAROON=<hex-encoded macaroon>
```

**Getting your LND REST URL on Umbrel:**
Your LND REST API is typically at `https://<umbrel-ip>:8080`. You may need to use your Umbrel's Tor address or local IP depending on your network setup.

**Creating a budget-limited macaroon (recommended):**

Instead of using your admin macaroon (which has full access to all funds), create a restricted one that can only spend a limited amount:

```bash
# SSH into your Umbrel, then exec into the LND container:
docker exec -it lightning_lnd_1 bash

# Create a macaroon that can only pay invoices (no on-chain, no channel management):
lncli bakemacaroon invoices:read invoices:write offchain:read offchain:write info:read --save_to /tmp/bolt402.macaroon

# Convert to hex:
xxd -p /tmp/bolt402.macaroon | tr -d '\n'
```

Copy the hex output and set it as `LND_MACAROON` in your `.env.local`.

This macaroon can pay Lightning invoices and check balance, but cannot:
- Send on-chain transactions
- Open/close channels
- Create invoices for receiving

The demo also enforces budget limits in code (`perRequestMax: 1000 sats`, `dailyMax: 50000 sats`) as an additional safety net.

**TLS certificate:**
If LND uses a self-signed certificate (common on Umbrel), you may need to set:
```bash
NODE_TLS_REJECT_UNAUTHORIZED=0
```
Or better, export your LND's `tls.cert` and configure it properly.

### Connecting SwissKnife

If you have a [Numeraire SwissKnife](https://github.com/bitcoin-numeraire/swissknife) instance:

```bash
BACKEND_TYPE=swissknife
SWISSKNIFE_URL=https://api.numeraire.tech
SWISSKNIFE_API_KEY=your_api_key
```

## Architecture

```
demos/l402-explorer/
├── src/
│   ├── app/
│   │   ├── page.tsx                  # Server component, fetches services from satring.com
│   │   ├── layout.tsx                # Root layout (dark theme)
│   │   ├── globals.css               # Tailwind + animations
│   │   └── api/
│   │       ├── chat/route.ts         # AI chat: streamText + createBolt402Tools()
│   │       └── l402-fetch/route.ts   # Manual L402 proxy for protocol flow visualizer
│   ├── components/
│   │   ├── ServiceBrowser.tsx        # Two-column layout, search, filters
│   │   ├── ServiceCard.tsx           # Individual service card
│   │   ├── ChatPanel.tsx             # AI chat with useChat, inline tool results
│   │   ├── ProtocolFlow.tsx          # Protocol flow visualization modal
│   │   └── SpendingDashboard.tsx     # Payment receipt tracker
│   └── lib/
│       ├── satring.ts                # satring.com API client
│       └── types.ts                  # UI type definitions
├── .env.example
├── .yarnrc.yml
├── package.json
└── tsconfig.json
```

bolt402-ai-sdk is linked from `../../packages/bolt402-ai-sdk` via `file:` dependency.

## How bolt402 is used

The chat API route (`/api/chat`) creates bolt402 tools and passes them to the Vercel AI SDK:

```typescript
import { createBolt402Tools, LndBackend } from 'bolt402-ai-sdk';
import { streamText } from 'ai';

const tools = createBolt402Tools({
  backend: new LndBackend({ url: LND_URL, macaroon: LND_MACAROON }),
  budget: { perRequestMax: 1000, dailyMax: 50000 },
});

const result = streamText({
  model: openai('gpt-4o'),
  tools,
  messages,
});
```

When the AI decides to call an L402 API, bolt402 handles the entire flow:
1. HTTP request to the endpoint
2. Receives 402 Payment Required + WWW-Authenticate header
3. Parses the L402 challenge (macaroon + invoice)
4. Pays the Lightning invoice via the configured backend
5. Retries the request with `Authorization: L402 <macaroon>:<preimage>`
6. Returns the data + payment receipt

## Scripts

```bash
yarn dev       # Development server (port 3000)
yarn build     # Production build
yarn start     # Start production server
yarn lint       # Run ESLint
```

## License

MIT OR Apache-2.0
