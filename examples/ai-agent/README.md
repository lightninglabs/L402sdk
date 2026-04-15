# AI Agent Example

Demonstrates an AI agent that autonomously pays for L402-gated APIs using the Vercel AI SDK and l402-ai-sdk.

## Prerequisites

- Node.js 22+
- An OpenAI API key (or any Vercel AI SDK-compatible provider)
- An LND node with REST API access (or a SwissKnife account)

## Setup

```bash
cd examples/ai-agent
yarn install
```

Create a `.env` file:

```env
OPENAI_API_KEY=sk-your-key
LND_URL=https://localhost:8080
LND_MACAROON=hex-encoded-admin-macaroon
```

## Run

```bash
yarn start
```

## What It Does

1. Creates an AI agent with L402 payment tools
2. Sets a budget (max 1,000 sats per request, 10,000 sats daily)
3. Asks the agent to fetch data from an L402-gated API
4. The agent automatically pays the Lightning invoice when it encounters a 402 response
5. Prints the result and payment receipts

## Architecture

```
User prompt
    ↓
Vercel AI SDK (generateText)
    ↓
AI Model (GPT-4o)
    ↓ tool call
l402_fetch tool (l402-ai-sdk)
    ↓
L402Client → 402 → pay invoice → retry → 200
    ↓
Result returned to AI model
    ↓
AI generates response
```

## Notes

- This example uses a real AI model and makes real API calls
- The L402 payment part requires either a real Lightning node or a mock setup
- For testing without real Lightning, see the [Getting Started tutorial](../../docs/tutorials/getting-started.md)
