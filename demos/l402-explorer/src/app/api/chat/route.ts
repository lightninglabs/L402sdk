import { streamText, stepCountIs, convertToModelMessages } from 'ai';
import { createMCPClient } from '@ai-sdk/mcp';
import { Experimental_StdioMCPTransport } from '@ai-sdk/mcp/mcp-stdio';
import { openai } from '@ai-sdk/openai';
import { anthropic, createAnthropic } from '@ai-sdk/anthropic';
import { xai } from '@ai-sdk/xai';
import {
  LndBackend,
  SwissKnifeBackend,
  type LnBackend,
} from 'bolt402-ai-sdk';
import { createBolt402ToolsV6 } from '@/lib/bolt402-tools';
import { MockBackend } from '@/lib/mock-backend';

// ---------------------------------------------------------------------------
// 402index MCP client (singleton — reused across requests)
// ---------------------------------------------------------------------------

let mcpClient: Awaited<ReturnType<typeof createMCPClient>> | null = null;
let mcpInitPromise: Promise<Awaited<ReturnType<typeof createMCPClient>>> | null = null;

async function getIndexMCPClient() {
  if (mcpClient) return mcpClient;
  if (mcpInitPromise) return mcpInitPromise;

  mcpInitPromise = (async () => {
    try {
      console.log('[bolt402-chat] Initializing 402index MCP client...');
      const client = await createMCPClient({
        transport: new Experimental_StdioMCPTransport({
          command: 'npx',
          args: ['-y', '@402index/mcp-server'],
        }),
      });
      mcpClient = client;
      console.log('[bolt402-chat] 402index MCP client ready');
      return client;
    } catch (err) {
      console.error('[bolt402-chat] Failed to init 402index MCP:', err);
      mcpInitPromise = null;
      throw err;
    }
  })();

  return mcpInitPromise;
}

// ---------------------------------------------------------------------------
// LLM provider detection
// ---------------------------------------------------------------------------

type Provider = 'openai' | 'anthropic' | 'xai';

function detectProvider(): { provider: Provider; model: string; apiKeySet: boolean } {
  if (process.env.ANTHROPIC_API_KEY || process.env.ANTHROPIC_AUTH_TOKEN) {
    return {
      provider: 'anthropic',
      model: process.env.AI_MODEL || 'claude-sonnet-4-20250514',
      apiKeySet: true,
    };
  }
  if (process.env.XAI_API_KEY) {
    return {
      provider: 'xai',
      model: process.env.AI_MODEL || 'grok-3-mini',
      apiKeySet: true,
    };
  }
  if (process.env.OPENAI_API_KEY) {
    return {
      provider: 'openai',
      model: process.env.AI_MODEL || process.env.OPENAI_MODEL || 'gpt-4o',
      apiKeySet: true,
    };
  }
  return { provider: 'openai', model: 'gpt-4o', apiKeySet: false };
}

function createModel(provider: Provider, model: string) {
  switch (provider) {
    case 'anthropic': {
      const authToken = process.env.ANTHROPIC_AUTH_TOKEN;
      if (authToken) {
        const p = createAnthropic({
          authToken,
          headers: { 'anthropic-beta': 'oauth-2025-04-20' },
        });
        return p(model);
      }
      return anthropic(model);
    }
    case 'xai':
      return xai(model);
    case 'openai':
    default:
      return openai(model);
  }
}

// ---------------------------------------------------------------------------
// Lightning backend
// ---------------------------------------------------------------------------

function createBackend(): LnBackend {
  const backendType = process.env.BACKEND_TYPE || 'mock';

  if (backendType === 'lnd' && process.env.LND_URL && process.env.LND_MACAROON) {
    return new LndBackend({
      url: process.env.LND_URL,
      macaroon: process.env.LND_MACAROON,
    });
  }

  if (
    backendType === 'swissknife' &&
    process.env.SWISSKNIFE_URL &&
    process.env.SWISSKNIFE_API_KEY
  ) {
    return new SwissKnifeBackend({
      url: process.env.SWISSKNIFE_URL,
      apiKey: process.env.SWISSKNIFE_API_KEY,
    });
  }

  return new MockBackend();
}

// ---------------------------------------------------------------------------
// System prompt (no static service list — agent discovers via MCP)
// ---------------------------------------------------------------------------

const SYSTEM_PROMPT = `You are an AI research assistant powered by bolt402. You help users query L402-gated APIs and pay for them with Lightning Network micropayments.

## Your tools

**Discovery (402index MCP):**
- search_services — Search the 402index directory for L402 APIs by keyword, category, health status, or price. Always filter by protocol "l402".
- get_service_detail — Get full details and health history for a specific service.
- list_categories — Browse available API categories.
- get_directory_stats — Get ecosystem overview (total services, health breakdown).

**Payment (bolt402):**
- l402_fetch — Fetch any URL, automatically paying Lightning invoices for L402-gated endpoints. This handles the full protocol: request → 402 challenge → pay invoice → retry with token.
- l402_get_balance — Check Lightning node balance before making payments.
- l402_get_receipts — Get payment receipts and total spending.

## Workflow

When a user asks a question:
1. Use search_services to find relevant L402 APIs (filter by protocol "l402", prefer "healthy" services)
2. Evaluate: check price, health status, and reliability score
3. Use l402_fetch to call the chosen endpoint and pay with Lightning
4. Present the data clearly with cost attribution (which API, sats spent, latency)

IMPORTANT: Many services list a base URL. You must call the specific endpoint path, not the base URL.
For example, call https://oracle.neofreight.net/api/price, NOT https://oracle.neofreight.net.

If you can't find a suitable API, explain what's available and suggest alternatives.
Always mention the cost of each API call to keep the user informed about spending.
Format data using markdown for clarity. Extract key information from JSON responses.`;

// ---------------------------------------------------------------------------
// Route handler
// ---------------------------------------------------------------------------

export async function POST(req: Request) {
  const { provider, model, apiKeySet } = detectProvider();
  const backendType = process.env.BACKEND_TYPE || 'mock';

  console.log('[bolt402-chat]', {
    provider,
    model,
    backend: backendType,
  });

  if (!apiKeySet) {
    return new Response(
      JSON.stringify({
        error:
          'No AI provider API key configured. Add ANTHROPIC_API_KEY, XAI_API_KEY, or OPENAI_API_KEY to .env.local.',
      }),
      { status: 500, headers: { 'Content-Type': 'application/json' } },
    );
  }

  try {
    const { messages } = await req.json();

    // Get bolt402 payment tools
    const backend = createBackend();
    const bolt402Tools = createBolt402ToolsV6({
      backend,
      budget: { perRequestMax: 1000, dailyMax: 50000 },
      maxFeeSats: 100,
    });

    // Get 402index MCP discovery tools (with fallback)
    let indexTools: Record<string, unknown> = {};
    try {
      const client = await getIndexMCPClient();
      indexTools = await client.tools();
      console.log('[bolt402-chat] MCP tools loaded:', Object.keys(indexTools).join(', '));
    } catch (err) {
      console.warn('[bolt402-chat] MCP unavailable, agent will use bolt402 tools only:', err);
    }

    const modelMessages = await convertToModelMessages(messages);

    const result = streamText({
      model: createModel(provider, model),
      system: SYSTEM_PROMPT,
      messages: modelMessages,
      tools: {
        ...indexTools,
        ...bolt402Tools,
      } as Parameters<typeof streamText>[0]['tools'],
      stopWhen: stepCountIs(8), // more steps: discover → evaluate → pay → present
      onError({ error }) {
        console.error('[bolt402-chat] Stream error:', error);
      },
    });

    return result.toUIMessageStreamResponse();
  } catch (error) {
    console.error('[bolt402-chat] Error:', error);
    return new Response(
      JSON.stringify({
        error: error instanceof Error ? error.message : 'Internal server error',
      }),
      { status: 500, headers: { 'Content-Type': 'application/json' } },
    );
  }
}
