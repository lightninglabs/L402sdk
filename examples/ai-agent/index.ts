/**
 * AI Agent Example — l402-ai-sdk + Vercel AI SDK
 *
 * Demonstrates an AI agent that can autonomously pay for L402-gated APIs.
 *
 * Usage:
 *   OPENAI_API_KEY=sk-... LND_URL=https://... LND_MACAROON=... npx tsx index.ts
 */

import { generateText, stepCountIs } from 'ai';
import { openai } from '@ai-sdk/openai';
import { createL402Tools, WasmBudgetConfig, WasmL402Client } from '@lightninglabs/l402-ai';

async function main() {
  console.log('L402sdk AI Agent Example');
  console.log('========================\n');

  // Step 1: Configure the Lightning client
  const client = WasmL402Client.withLndRest(
    process.env.LND_URL ?? 'https://localhost:8080',
    process.env.LND_MACAROON ?? '',
    new WasmBudgetConfig(1_000, 0, 10_000, 0),
    100,
  );

  // Step 2: Create tools with the configured client
  const tools = createL402Tools({ client });

  console.log('Lightning client configured');
  console.log('Budget: max 1,000 sats/request, 10,000 sats/day\n');

  // Step 3: Run the agent
  const prompt =
    'Fetch the premium weather data from https://api.example.com/v1/weather. ' +
    'If the API requires payment, pay for it. ' +
    'After fetching, tell me the total cost.';

  console.log(`Prompt: ${prompt}\n`);
  console.log('Running agent...\n');

  try {
    const result = await generateText({
      model: openai('gpt-4o'),
      tools,
      stopWhen: stepCountIs(5),
      prompt,
    });

    console.log('Agent response:');
    console.log(result.text);
    console.log();

    // Step 4: Show tool usage
    if (result.steps.length > 0) {
      console.log('Tool calls made:');
      for (const step of result.steps) {
        for (const call of step.toolCalls) {
          console.log(`  - ${call.toolName}(${JSON.stringify(call.args)})`);
        }
      }
    }
  } catch (error) {
    console.error('Agent error:', error);
  }
}

main().catch(console.error);
