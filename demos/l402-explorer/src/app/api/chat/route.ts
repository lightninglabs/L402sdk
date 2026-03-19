import { streamText, stepCountIs, convertToModelMessages } from 'ai';
import { openai } from '@ai-sdk/openai';
import { anthropic, createAnthropic } from '@ai-sdk/anthropic';
import { xai } from '@ai-sdk/xai';
import {
  createBolt402Tools,
  LndBackend,
  SwissKnifeBackend,
  type LnBackend,
} from 'bolt402-ai-sdk';
import { MockBackend } from '@/lib/mock-backend';

type Provider = 'openai' | 'anthropic' | 'xai';

function detectProvider(): { provider: Provider; model: string; apiKeySet: boolean } {
  // Auto-detect provider from environment (priority: Anthropic > xAI > OpenAI)
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
      // Support both standard API keys (ANTHROPIC_API_KEY) and OAuth tokens
      // (ANTHROPIC_AUTH_TOKEN) from `claude setup-token`. OAuth tokens require
      // the oauth beta header.
      const authToken = process.env.ANTHROPIC_AUTH_TOKEN;
      if (authToken) {
        const provider = createAnthropic({
          authToken,
          headers: { 'anthropic-beta': 'oauth-2025-04-20' },
        });
        return provider(model);
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

function getConfig() {
  const backendType = process.env.BACKEND_TYPE || 'mock';
  const { provider, model, apiKeySet } = detectProvider();
  const lndUrl = process.env.LND_URL || '(not set)';
  const swissKnifeUrl = process.env.SWISSKNIFE_URL || '(not set)';
  const indexUrl = process.env.INDEX_API_URL || 'https://402index.io/api/v1';

  return { backendType, provider, model, apiKeySet, lndUrl, swissKnifeUrl, indexUrl };
}

function createBackend(): LnBackend {
  const { backendType } = getConfig();

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

function buildSystemPrompt(services: Array<{ name: string; url: string; description: string; price_sats: number | null; category: string; provider: string }>) {
  const serviceList = services
    .map((s) => {
      const price = s.price_sats != null ? `${s.price_sats} sats` : 'unknown';
      return `- **${s.name}**: ${s.description || 'No description'}\n  URL: ${s.url}\n  Price: ${price}\n  Category: ${s.category}\n  Provider: ${s.provider || 'Unknown'}`;
    })
    .join('\n');

  return `You are an AI research assistant powered by bolt402. You have access to L402-gated APIs that you can query by paying with Lightning Network micropayments.

Available L402 services:
${serviceList || 'No services currently loaded.'}

When a user asks a question:
1. Identify which L402 API(s) can answer it
2. Use the l402_fetch tool to call the specific API endpoint URL (not just the base URL)
3. Present the data clearly and in a well-formatted way
4. Report which APIs you used, their cost in sats, and response latency

IMPORTANT: Many services list a base URL. You must call the specific endpoint path, not the base URL.
For example, call https://oracle.neofreight.net/api/price, NOT https://oracle.neofreight.net.

If no API can answer the question, explain what services are available and what they can do.
Always mention the cost of each API call to keep the user informed about spending.

When presenting data, use markdown formatting for clarity. If you receive JSON data, extract the key information and present it in a human-readable format.`;
}

export async function POST(req: Request) {
  const config = getConfig();

  if (!config.apiKeySet) {
    return new Response(
      JSON.stringify({
        error:
          'No AI provider API key configured. Add ANTHROPIC_API_KEY, XAI_API_KEY, or OPENAI_API_KEY to .env.local.',
      }),
      { status: 500, headers: { 'Content-Type': 'application/json' } },
    );
  }

  try {
    const { messages, services } = await req.json();

    const backend = createBackend();
    const tools = createBolt402Tools({
      backend,
      budget: { perRequestMax: 1000, dailyMax: 50000 },
      maxFeeSats: 100,
    });

    const modelMessages = await convertToModelMessages(messages);

    const result = streamText({
      model: createModel(config.provider, config.model),
      system: buildSystemPrompt(services || []),
      messages: modelMessages,
      tools,
      stopWhen: stepCountIs(5),
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
