'use client';

import { useChat } from '@ai-sdk/react';
import { DefaultChatTransport } from 'ai';
import { useState, useRef, useEffect, useCallback } from 'react';
import type { L402Service } from '@/lib/types';

interface ChatPanelProps {
  services: L402Service[];
  onSpend: (entry: {
    service: string;
    url: string;
    amountSats: number;
    feeSats: number;
    latencyMs: number;
    status: number;
  }) => void;
}

interface ToolInvocationResult {
  url?: string;
  status?: number;
  paid?: boolean;
  receipt?: {
    amountSats: number;
    feeSats: number;
    totalCostSats: number;
    paymentHash: string;
    latencyMs: number;
  } | null;
  body?: string;
  // l402_get_balance
  balanceSats?: number;
  nodeAlias?: string;
  // l402_get_receipts
  totalSpentSats?: number;
  paymentCount?: number;
  receipts?: Array<{
    url: string;
    amountSats: number;
    totalCostSats: number;
    latencyMs: number;
  }>;
}

const SUGGESTIONS = [
  "What's the current Bitcoin price?",
  'Show me US CPI data',
  "What's the Brent crude oil price?",
];

export default function ChatPanel({ services, onSpend }: ChatPanelProps) {
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const [hasInteracted, setHasInteracted] = useState(false);
  const [input, setInput] = useState('');

  const [chatTransport] = useState(
    () =>
      new DefaultChatTransport({
        api: '/api/chat',
      }),
  );

  const { messages, sendMessage, status, error } = useChat({
    transport: chatTransport,
    onError(err: Error) {
      console.error('[chat] Error:', err);
    },
  });

  const isLoading = status === 'streaming' || status === 'submitted';

  // Track spending from completed tool invocations
  useEffect(() => {
    for (const msg of messages) {
      if (msg.role !== 'assistant') continue;
      const parts = msg.parts;
      if (!parts) continue;
      for (const part of parts) {
        if (!part.type.startsWith('tool-')) continue;
        if (!('state' in part) || part.state !== 'output-available') continue;
        if (!('toolName' in part)) continue;
        const toolPart = part as { toolName: string; state: string; output: unknown; input: unknown };
        const result = toolPart.output as ToolInvocationResult;
        if (toolPart.toolName === 'l402_fetch' && result?.receipt) {
          const key = `${result.receipt.paymentHash}`;
          if (!(window as unknown as Record<string, unknown>)[`__spent_${key}`]) {
            (window as unknown as Record<string, unknown>)[`__spent_${key}`] = true;
            const urlStr = result.url || '';
            const matchedService = services.find((s) => urlStr.startsWith(s.url));
            onSpend({
              service: matchedService?.name || 'Unknown',
              url: urlStr,
              amountSats: result.receipt.amountSats,
              feeSats: result.receipt.feeSats,
              latencyMs: result.receipt.latencyMs,
              status: result.status || 200,
            });
          }
        }
      }
    }
  }, [messages, services, onSpend]);

  // Auto-scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handleSuggestion = useCallback(
    (text: string) => {
      setHasInteracted(true);
      sendMessage({ text });
    },
    [sendMessage],
  );

  const onFormSubmit = useCallback(
    (e: React.FormEvent<HTMLFormElement>) => {
      e.preventDefault();
      const trimmed = input.trim();
      if (!trimmed) return;
      setHasInteracted(true);
      setInput('');
      sendMessage({ text: trimmed });
    },
    [input, sendMessage],
  );

  return (
    <div className="flex flex-col h-full rounded-xl border border-zinc-800 bg-zinc-900/50 overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 border-b border-zinc-800 px-4 py-3 shrink-0">
        <span className="text-base">🤖</span>
        <div>
          <h3 className="text-sm font-semibold text-zinc-100">AI Research Assistant</h3>
          <p className="text-[10px] text-zinc-500">Powered by bolt402 &middot; Pays for APIs with Lightning</p>
        </div>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto px-4 py-4 space-y-4 min-h-0">
        {!hasInteracted && messages.length === 0 && (
          <div className="flex flex-col items-center justify-center h-full text-center py-8">
            <span className="text-4xl mb-3">⚡</span>
            <h4 className="text-sm font-semibold text-zinc-200 mb-1">Ask anything</h4>
            <p className="text-xs text-zinc-500 max-w-xs mb-6">
              I&apos;ll query L402 APIs and pay with Lightning micropayments to get you answers.
            </p>
            <div className="flex flex-wrap justify-center gap-2">
              {SUGGESTIONS.map((s) => (
                <button
                  key={s}
                  onClick={() => handleSuggestion(s)}
                  className="rounded-lg border border-zinc-700 bg-zinc-800/50 px-3 py-1.5 text-xs text-zinc-300 hover:border-[#F7931A]/40 hover:text-[#F7931A] transition-colors"
                >
                  {s}
                </button>
              ))}
            </div>
          </div>
        )}

        {messages.map((msg) => (
          <div key={msg.id} className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}>
            <div
              className={`max-w-[85%] rounded-xl px-3.5 py-2.5 text-sm leading-relaxed ${
                msg.role === 'user'
                  ? 'bg-[#F7931A]/15 text-zinc-100 border border-[#F7931A]/20'
                  : 'bg-zinc-800/70 text-zinc-200 border border-zinc-700/50'
              }`}
            >
              {/* Render parts */}
              {msg.parts?.map((part, i) => {
                if (part.type === 'text') {
                  return (
                    <div key={i} className="whitespace-pre-wrap break-words prose-sm prose-invert">
                      {part.text}
                    </div>
                  );
                }

                if (part.type.startsWith('tool-')) {
                  const toolPart = part as { type: string; toolName: string; state: string; input: unknown; output?: unknown };
                  return (
                    <ToolCallDisplay
                      key={i}
                      toolName={toolPart.toolName}
                      state={toolPart.state}
                      args={(toolPart.input ?? {}) as Record<string, unknown>}
                      result={toolPart.state === 'output-available' ? (toolPart.output as ToolInvocationResult) : undefined}
                      services={services}
                    />
                  );
                }

                return null;
              })}

              {/* Fallback for messages without parts */}
              {(!msg.parts || msg.parts.length === 0) && (
                <div className="whitespace-pre-wrap break-words text-zinc-400">...</div>
              )}
            </div>
          </div>
        ))}

        {isLoading && messages[messages.length - 1]?.role === 'user' && (
          <div className="flex justify-start">
            <div className="bg-zinc-800/70 border border-zinc-700/50 rounded-xl px-3.5 py-2.5">
              <div className="flex items-center gap-1.5">
                <div className="h-1.5 w-1.5 rounded-full bg-[#F7931A] animate-bounce [animation-delay:0ms]" />
                <div className="h-1.5 w-1.5 rounded-full bg-[#F7931A] animate-bounce [animation-delay:150ms]" />
                <div className="h-1.5 w-1.5 rounded-full bg-[#F7931A] animate-bounce [animation-delay:300ms]" />
              </div>
            </div>
          </div>
        )}

        {error && (
          <div className="mx-1 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-400">
            <span className="font-medium">Error:</span> {error.message || 'Something went wrong. Check your API key in .env.local.'}
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <form
        id="chat-form"
        onSubmit={onFormSubmit}
        className="shrink-0 border-t border-zinc-800 px-4 py-3"
      >
        <div className="flex items-center gap-2">
          <input
            ref={inputRef}
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="Ask about Bitcoin price, economic data..."
            disabled={isLoading}
            className="flex-1 rounded-lg border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-100 placeholder:text-zinc-500 focus:border-[#F7931A]/50 focus:outline-none focus:ring-1 focus:ring-[#F7931A]/30 disabled:opacity-50 transition-colors"
          />
          <button
            type="submit"
            disabled={isLoading || !input.trim()}
            className="rounded-lg bg-[#F7931A] px-3 py-2 text-sm font-medium text-zinc-950 hover:bg-[#F7931A]/90 disabled:opacity-40 disabled:cursor-not-allowed transition-colors shrink-0"
          >
            ⚡ Send
          </button>
        </div>
      </form>
    </div>
  );
}

/** Renders a tool invocation inline within a message. */
function ToolCallDisplay({
  toolName,
  state,
  args,
  result,
  services,
}: {
  toolName: string;
  state: string;
  args: Record<string, unknown>;
  result?: ToolInvocationResult;
  services: L402Service[];
}) {
  const isRunning = state === 'input-streaming' || state === 'input-available' || state === 'streaming';

  if (toolName === 'l402_fetch') {
    const url = (args.url as string) || '';
    const matchedService = services.find((s) => url.startsWith(s.url));

    return (
      <div className="my-2 rounded-lg border border-zinc-700/50 bg-zinc-900/80 p-2.5 text-xs">
        <div className="flex items-center gap-2 mb-1.5">
          <span className="text-[#F7931A]">⚡</span>
          <span className="font-medium text-zinc-300">
            {isRunning ? 'Fetching...' : 'L402 API Call'}
          </span>
          {isRunning && (
            <svg className="h-3 w-3 animate-spin text-[#F7931A]" viewBox="0 0 24 24" fill="none">
              <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="2" className="opacity-25" />
              <path d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" fill="currentColor" className="opacity-75" />
            </svg>
          )}
        </div>

        {/* URL */}
        <div className="font-mono text-[10px] text-zinc-500 truncate mb-1">{url}</div>

        {/* Service attribution */}
        {matchedService && (
          <div className="text-[10px] text-zinc-400 mb-1.5">
            Service: <span className="text-[#F7931A]">{matchedService.name}</span>
            <span className="text-zinc-600"> &middot; {matchedService.price_sats ?? '?'} sats</span>
          </div>
        )}

        {/* Result */}
        {result && (
          <div className="flex items-center gap-3 text-[10px] border-t border-zinc-800 pt-1.5 mt-1">
            <span className={result.paid ? 'text-emerald-400' : 'text-zinc-500'}>
              {result.paid ? '✓ Paid' : 'Free'}
            </span>
            {result.receipt && (
              <>
                <span className="text-[#F7931A] font-mono">
                  {result.receipt.totalCostSats} sats
                </span>
                <span className="text-zinc-500 font-mono">
                  {result.receipt.latencyMs}ms
                </span>
              </>
            )}
            <span
              className={`font-mono ${
                result.status && result.status < 400 ? 'text-emerald-400' : 'text-red-400'
              }`}
            >
              {result.status}
            </span>
          </div>
        )}
      </div>
    );
  }

  if (toolName === 'l402_get_balance') {
    return (
      <div className="my-2 rounded-lg border border-zinc-700/50 bg-zinc-900/80 p-2.5 text-xs">
        <div className="flex items-center gap-2">
          <span>💰</span>
          <span className="font-medium text-zinc-300">
            {isRunning ? 'Checking balance...' : 'Node Balance'}
          </span>
        </div>
        {result && (
          <div className="mt-1.5 text-[10px] text-zinc-400">
            <span className="text-[#F7931A] font-mono">{result.balanceSats?.toLocaleString()} sats</span>
            <span className="text-zinc-600"> &middot; {result.nodeAlias}</span>
          </div>
        )}
      </div>
    );
  }

  if (toolName === 'l402_get_receipts') {
    return (
      <div className="my-2 rounded-lg border border-zinc-700/50 bg-zinc-900/80 p-2.5 text-xs">
        <div className="flex items-center gap-2">
          <span>🧾</span>
          <span className="font-medium text-zinc-300">
            {isRunning ? 'Loading receipts...' : 'Payment Receipts'}
          </span>
        </div>
        {result && (
          <div className="mt-1.5 text-[10px] text-zinc-400">
            <span className="text-[#F7931A] font-mono">{result.totalSpentSats} sats</span>
            <span className="text-zinc-600"> across {result.paymentCount} payments</span>
          </div>
        )}
      </div>
    );
  }

  // 402index MCP tools
  if (toolName === 'search_services') {
    const query = (args.q as string) || (args.category as string) || 'L402 services';
    return (
      <div className="my-2 rounded-lg border border-zinc-700/50 bg-zinc-900/80 p-2.5 text-xs">
        <div className="flex items-center gap-2 mb-1">
          <span>🔍</span>
          <span className="font-medium text-zinc-300">
            {isRunning ? 'Searching services...' : 'Service Discovery'}
          </span>
          {isRunning && (
            <svg className="h-3 w-3 animate-spin text-[#F7931A]" viewBox="0 0 24 24" fill="none">
              <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="2" className="opacity-25" />
              <path d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" fill="currentColor" className="opacity-75" />
            </svg>
          )}
        </div>
        <div className="text-[10px] text-zinc-500">Query: {query}</div>
        {result && (
          <div className="mt-1 text-[10px] text-zinc-400">
            Found services via 402index.io
          </div>
        )}
      </div>
    );
  }

  if (toolName === 'get_service_detail') {
    return (
      <div className="my-2 rounded-lg border border-zinc-700/50 bg-zinc-900/80 p-2.5 text-xs">
        <div className="flex items-center gap-2">
          <span>📋</span>
          <span className="font-medium text-zinc-300">
            {isRunning ? 'Checking service details...' : 'Service Detail'}
          </span>
        </div>
      </div>
    );
  }

  if (toolName === 'list_categories' || toolName === 'get_directory_stats') {
    return (
      <div className="my-2 rounded-lg border border-zinc-700/50 bg-zinc-900/80 p-2.5 text-xs">
        <div className="flex items-center gap-2">
          <span>📊</span>
          <span className="font-medium text-zinc-300">
            {isRunning ? `Running ${toolName}...` : toolName.replace(/_/g, ' ')}
          </span>
        </div>
      </div>
    );
  }

  // Generic fallback
  return (
    <div className="my-2 rounded-lg border border-zinc-700/50 bg-zinc-900/80 p-2.5 text-xs">
      <span className="text-zinc-400">{isRunning ? `Running ${toolName}...` : toolName}</span>
    </div>
  );
}
