import { streamText, stepCountIs, convertToModelMessages, jsonSchema } from 'ai';
import { createMCPClient } from '@ai-sdk/mcp';
import { Experimental_StdioMCPTransport } from '@ai-sdk/mcp/mcp-stdio';
import { openai } from '@ai-sdk/openai';
import { anthropic, createAnthropic } from '@ai-sdk/anthropic';
import { xai } from '@ai-sdk/xai';
import { createBolt402Tools } from 'bolt402-ai-sdk';
import { getSharedL402Client } from '@/lib/l402-shared';

// ---------------------------------------------------------------------------
// 402index MCP client (singleton — reused across requests)
// ---------------------------------------------------------------------------

let mcpClient: Awaited<ReturnType<typeof createMCPClient>> | null = null;
let mcpInitPromise: Promise<Awaited<ReturnType<typeof createMCPClient>>> | null = null;
let cachedCategories: string[] = [];

async function getIndexMCPClient() {
  if (mcpClient) return mcpClient;
  if (mcpInitPromise) return mcpInitPromise;

  mcpInitPromise = (async () => {
    try {
      console.log('[bolt402-chat] Initializing 402index MCP client...');
      const client = await createMCPClient({
        transport: new Experimental_StdioMCPTransport({
          command: 'npx',
          args: ['-y', '@402index/mcp-server@0.2.4'],
        }),
      });
      mcpClient = client;
      console.log('[bolt402-chat] 402index MCP client ready');

      // Pre-fetch valid categories so the model knows what exists
      try {
        const tools = await client.tools();
        if (tools.list_categories) {
          const result = await tools.list_categories.execute({}, { toolCallId: '_init', messages: [] });
          const parsed = extractMCPText(result);
          if (parsed?.categories && typeof parsed.categories === 'object') {
            cachedCategories = Object.keys(parsed.categories);
            console.log('[bolt402-chat] Cached categories:', cachedCategories.join(', '));
          }
        }
      } catch (err) {
        console.warn('[bolt402-chat] Failed to pre-fetch categories:', err);
      }

      return client;
    } catch (err) {
      console.error('[bolt402-chat] Failed to init 402index MCP:', err);
      mcpInitPromise = null;
      throw err;
    }
  })();

  return mcpInitPromise;
}

/** Extract parsed JSON from an MCP tool result's text content. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function extractMCPText(result: unknown): any {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const r = result as any;
  if (r?.content) {
    const text = r.content.find((c: { type: string }) => c.type === 'text');
    if (text?.text) {
      try { return JSON.parse(text.text); } catch { return null; }
    }
  }
  return null;
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
// MCP tool schema filtering
// ---------------------------------------------------------------------------

/**
 * Strip optional properties from an MCP tool's JSON Schema so the LLM only
 * sees the fields we want it to use. Models like gpt-5.4 fill in every
 * schema property (e.g. source: "discovery", featured: true) which
 * over-filters search results.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function filterToolSchema(tool: any, allowedProps: string[]): unknown {
  const schema = tool.inputSchema?.jsonSchema;
  if (!schema?.properties) return tool;

  const filteredSchema = {
    ...schema,
    properties: Object.fromEntries(
      Object.entries(schema.properties).filter(([key]: [string, unknown]) => allowedProps.includes(key)),
    ),
  };

  if (Array.isArray(filteredSchema.required)) {
    filteredSchema.required = filteredSchema.required.filter(
      (r: string) => allowedProps.includes(r),
    );
  }

  return {
    ...tool,
    inputSchema: jsonSchema(filteredSchema),
  };
}

// ---------------------------------------------------------------------------
// System prompt (no static service list — agent discovers via MCP)
// ---------------------------------------------------------------------------

function buildSystemPrompt(): string {
  const categoryList = cachedCategories.length > 0
    ? `\n\nKnown service categories in the directory: ${cachedCategories.join(', ')}.`
    : '';

  return `You are an AI research assistant powered by bolt402. You help users query L402-gated APIs and pay for them with Lightning Network micropayments.

## Your tools

**Discovery (402index MCP):**
- search_services — Search the 402index directory for services by keyword.
- get_service_detail — Get full details for a specific service by ID.
- list_categories — Browse available API categories.
- get_directory_stats — Get ecosystem overview.

**Payment (bolt402):**
- l402_fetch — Fetch any URL, automatically paying Lightning invoices for L402 endpoints. Handles the full L402 flow.
- l402_get_balance — Check Lightning node balance.
- l402_get_receipts — Get payment receipts and spending totals.
${categoryList}
## Workflow

When a user asks a question:
1. Use search_services with q (keyword), protocol="L402", and limit=200. The keyword search is fuzzy — use simple, broad terms (e.g. "twitter", "bitcoin price", "weather").
2. If no results, retry with different keywords: synonyms, shorter terms, or related concepts.
3. Pick the best service based on health, reliability_score, and latency.
4. Use l402_fetch to call the chosen endpoint URL and pay with Lightning.
5. Present the data clearly with cost attribution.

## Important rules

- Many services list a base URL. You MUST call the specific endpoint path, not the base URL.
  Example: call https://oracle.neofreight.net/api/price, NOT https://oracle.neofreight.net.
- Always mention the cost of each API call.
- Format responses using markdown.
- You CAN use l402_fetch on any URL — you don't need to discover it first if the user provides one.`;
}

// ---------------------------------------------------------------------------
// Route handler
// ---------------------------------------------------------------------------

export async function POST(req: Request) {
  const { provider, model, apiKeySet } = detectProvider();

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

    // Get bolt402 payment tools (shared client so receipts persist)
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const bolt402Tools = createBolt402Tools({
      client: getSharedL402Client() as any,
    });

    // Get 402index MCP discovery tools (with fallback)
    let indexTools: Record<string, unknown> = {};
    try {
      const client = await getIndexMCPClient();
      indexTools = await client.tools();

      // Strip problematic optional fields from search_services schema.
      // Models like GPT-5.4 fill in every schema property with guessed
      // values (e.g. category: "social", health: "healthy") which
      // over-filters and returns 0 results.
      // Only expose category if we have valid values to constrain it.
      if (indexTools.search_services) {
        indexTools.search_services = filterToolSchema(
          indexTools.search_services,
          ['q', 'protocol', 'limit'],
        );
      }
    } catch (err) {
      console.warn('[bolt402-chat] MCP unavailable, agent will use bolt402 tools only:', err);
    }

    const modelMessages = await convertToModelMessages(messages);

    const allTools = {
      ...indexTools,
      ...bolt402Tools,
    } as Parameters<typeof streamText>[0]['tools'];

    const result = streamText({
      model: createModel(provider, model),
      system: buildSystemPrompt(),
      messages: modelMessages,
      tools: allTools,
      stopWhen: stepCountIs(8),
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
