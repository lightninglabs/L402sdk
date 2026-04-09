import { highlight } from "@/lib/highlight";
import {
  lngetSnippets,
  bolt402Snippets,
  featureComparison,
} from "@/lib/snippets";
import LanguageTabs from "@/components/LanguageTabs";
import FeatureTable from "@/components/FeatureTable";

/** Pre-render all Shiki highlights at build time. */
async function renderTabs(
  snippets: { label: string; code: string; shikiLang: string }[],
) {
  return Promise.all(
    snippets.map(async (s) => ({
      label: s.label,
      html: await highlight(s.code, s.shikiLang),
    })),
  );
}

export default async function Home() {
  const [lngetTabs, bolt402Tabs] = await Promise.all([
    renderTabs(lngetSnippets),
    renderTabs(bolt402Snippets),
  ]);

  return (
    <div className="flex flex-col min-h-screen">
      {/* Header */}
      <header className="sticky top-0 z-40 border-b border-zinc-800 bg-zinc-950/80 backdrop-blur-md">
        <div className="mx-auto flex max-w-7xl items-center justify-between px-4 py-3 sm:px-6">
          <div className="flex items-center gap-2.5">
            <span className="text-xl">⚡</span>
            <div>
              <h1 className="text-sm font-bold text-zinc-100 tracking-tight">
                bolt402 vs lnget
              </h1>
              <p className="text-[10px] text-zinc-500">
                SDK comparison
              </p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <a
              href="https://github.com/lightninglabs/bolt402"
              target="_blank"
              rel="noopener noreferrer"
              className="rounded-lg border border-zinc-800 px-3 py-1.5 text-xs text-zinc-400 hover:border-zinc-600 hover:text-zinc-200 transition-colors"
            >
              GitHub
            </a>
            <a
              href="https://github.com/lightninglabs/lnget"
              target="_blank"
              rel="noopener noreferrer"
              className="rounded-lg border border-zinc-800 px-3 py-1.5 text-xs text-zinc-400 hover:border-zinc-600 hover:text-zinc-200 transition-colors"
            >
              lnget
            </a>
          </div>
        </div>
      </header>

      {/* Hero */}
      <section className="border-b border-zinc-800 bg-zinc-950">
        <div className="mx-auto max-w-7xl px-4 py-10 sm:px-6 sm:py-14">
          <div className="max-w-3xl">
            <h2 className="text-2xl font-bold text-zinc-100 tracking-tight sm:text-3xl">
              CLI binary vs. embedded SDK
            </h2>
            <p className="mt-3 text-sm leading-relaxed text-zinc-400 max-w-2xl">
              Both tools solve the same problem: automatically paying L402 invoices
              so your code can access Lightning-gated APIs. The difference is
              how they integrate.{" "}
              <span className="text-zinc-300">lnget</span> is a standalone CLI
              you shell out to.{" "}
              <span className="text-[#F7931A]">bolt402</span> is a library you
              import and call directly, with native types, pluggable backends,
              and built-in AI framework tooling.
            </p>
            <div className="mt-5 flex flex-wrap items-center gap-3 text-xs text-zinc-500">
              <span className="inline-flex items-center gap-1.5 rounded-full bg-zinc-800 px-3 py-1">
                <span className="h-1.5 w-1.5 rounded-full bg-[#F7931A]" />
                Rust + TypeScript + Python
              </span>
              <span className="inline-flex items-center gap-1.5 rounded-full bg-zinc-800 px-3 py-1">
                <span className="h-1.5 w-1.5 rounded-full bg-emerald-500" />
                Vercel AI SDK native
              </span>
              <span className="inline-flex items-center gap-1.5 rounded-full bg-zinc-800 px-3 py-1">
                <span className="h-1.5 w-1.5 rounded-full bg-violet-500" />
                Pluggable backends
              </span>
            </div>
          </div>
        </div>
      </section>

      {/* Side-by-side comparison */}
      <section className="border-b border-zinc-800">
        <div className="mx-auto max-w-7xl px-4 py-8 sm:px-6 sm:py-12">
          <h3 className="text-lg font-semibold text-zinc-100 mb-6">
            Same endpoint. Different integration.
          </h3>

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            {/* lnget side */}
            <div className="rounded-xl border border-zinc-800 bg-zinc-900/30 overflow-hidden">
              <div className="flex items-center gap-2 border-b border-zinc-800 px-4 py-3">
                <span className="text-base">🔧</span>
                <div>
                  <h4 className="text-sm font-semibold text-zinc-200">lnget</h4>
                  <p className="text-[10px] text-zinc-500">
                    CLI binary, shell out from your code
                  </p>
                </div>
              </div>
              <div className="p-4">
                <LanguageTabs tabs={lngetTabs} />

                <div className="mt-5 space-y-2.5">
                  <h5 className="text-xs font-semibold text-zinc-400 uppercase tracking-wider">
                    Integration overhead
                  </h5>
                  <ul className="space-y-1.5 text-xs text-zinc-500">
                    <li className="flex items-start gap-2">
                      <span className="text-red-400 mt-0.5">✕</span>
                      Subprocess management and lifecycle
                    </li>
                    <li className="flex items-start gap-2">
                      <span className="text-red-400 mt-0.5">✕</span>
                      Parse JSON strings from stdout
                    </li>
                    <li className="flex items-start gap-2">
                      <span className="text-red-400 mt-0.5">✕</span>
                      Exit codes for error handling
                    </li>
                    <li className="flex items-start gap-2">
                      <span className="text-red-400 mt-0.5">✕</span>
                      Binary must be installed on the host
                    </li>
                    <li className="flex items-start gap-2">
                      <span className="text-red-400 mt-0.5">✕</span>
                      No programmatic budget or receipt access
                    </li>
                  </ul>
                </div>
              </div>
            </div>

            {/* bolt402 side */}
            <div className="rounded-xl border border-[#F7931A]/30 bg-zinc-900/30 overflow-hidden">
              <div className="flex items-center gap-2 border-b border-[#F7931A]/20 px-4 py-3">
                <span className="text-base">⚡</span>
                <div>
                  <h4 className="text-sm font-semibold text-[#F7931A]">
                    bolt402
                  </h4>
                  <p className="text-[10px] text-zinc-500">
                    Embedded SDK, native import
                  </p>
                </div>
              </div>
              <div className="p-4">
                <LanguageTabs tabs={bolt402Tabs} />

                <div className="mt-5 space-y-2.5">
                  <h5 className="text-xs font-semibold text-zinc-400 uppercase tracking-wider">
                    Developer experience
                  </h5>
                  <ul className="space-y-1.5 text-xs text-zinc-500">
                    <li className="flex items-start gap-2">
                      <span className="text-emerald-400 mt-0.5">✓</span>
                      In-process, zero subprocess overhead
                    </li>
                    <li className="flex items-start gap-2">
                      <span className="text-emerald-400 mt-0.5">✓</span>
                      Typed responses and error variants
                    </li>
                    <li className="flex items-start gap-2">
                      <span className="text-emerald-400 mt-0.5">✓</span>
                      Pluggable token stores (memory, file, localStorage)
                    </li>
                    <li className="flex items-start gap-2">
                      <span className="text-emerald-400 mt-0.5">✓</span>
                      Programmatic budget with per-domain limits
                    </li>
                    <li className="flex items-start gap-2">
                      <span className="text-emerald-400 mt-0.5">✓</span>
                      Native Vercel AI SDK tool integration
                    </li>
                  </ul>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Architecture Diagram */}
      <section className="border-b border-zinc-800">
        <div className="mx-auto max-w-7xl px-4 py-8 sm:px-6 sm:py-12">
          <h3 className="text-lg font-semibold text-zinc-100 mb-6">
            Architecture comparison
          </h3>

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            {/* lnget architecture */}
            <div className="rounded-xl border border-zinc-800 bg-zinc-900/30 p-5">
              <h4 className="text-sm font-semibold text-zinc-300 mb-4">
                🔧 lnget: external process
              </h4>
              <div className="font-mono text-xs text-zinc-500 space-y-2 leading-relaxed">
                <div className="rounded bg-zinc-800/60 p-3">
                  <span className="text-zinc-400">Your App</span>
                  <br />
                  &nbsp;&nbsp;├─ exec(&quot;lnget ...&quot;)
                  <br />
                  &nbsp;&nbsp;│&nbsp;&nbsp;└─{" "}
                  <span className="text-yellow-400">subprocess</span>
                  <br />
                  &nbsp;&nbsp;│&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;├─ HTTP 402 →
                  parse challenge
                  <br />
                  &nbsp;&nbsp;│&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;├─ LND gRPC →
                  pay invoice
                  <br />
                  &nbsp;&nbsp;│&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;├─ Retry with
                  L402 token
                  <br />
                  &nbsp;&nbsp;│&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;└─ stdout →{" "}
                  <span className="text-yellow-400">JSON string</span>
                  <br />
                  &nbsp;&nbsp;├─ parse(stdout)
                  <br />
                  &nbsp;&nbsp;└─ use data
                </div>
                <p className="text-zinc-600 text-[10px] mt-2">
                  Cross-process boundary. String serialization. No shared state.
                </p>
              </div>
            </div>

            {/* bolt402 architecture */}
            <div className="rounded-xl border border-[#F7931A]/30 bg-zinc-900/30 p-5">
              <h4 className="text-sm font-semibold text-[#F7931A] mb-4">
                ⚡ bolt402: in-process SDK
              </h4>
              <div className="font-mono text-xs text-zinc-500 space-y-2 leading-relaxed">
                <div className="rounded bg-zinc-800/60 p-3">
                  <span className="text-zinc-400">Your App</span>
                  <br />
                  &nbsp;&nbsp;├─ client.get(url)
                  <br />
                  &nbsp;&nbsp;│&nbsp;&nbsp;└─{" "}
                  <span className="text-emerald-400">in-process</span>
                  <br />
                  &nbsp;&nbsp;│&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;├─ HTTP 402 →
                  parse challenge
                  <br />
                  &nbsp;&nbsp;│&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;├─{" "}
                  <span className="text-emerald-400">LnBackend</span> trait →
                  pay invoice
                  <br />
                  &nbsp;&nbsp;│&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;├─{" "}
                  <span className="text-emerald-400">TokenStore</span> trait →
                  cache token
                  <br />
                  &nbsp;&nbsp;│&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;├─{" "}
                  <span className="text-emerald-400">BudgetTracker</span> →
                  enforce limits
                  <br />
                  &nbsp;&nbsp;│&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;└─{" "}
                  <span className="text-emerald-400">Response&lt;T&gt;</span>
                  <br />
                  &nbsp;&nbsp;└─ response.json::&lt;Weather&gt;()
                </div>
                <p className="text-zinc-600 text-[10px] mt-2">
                  Same process. Native types. Pluggable ports and adapters.
                </p>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Feature comparison table */}
      <section className="border-b border-zinc-800">
        <div className="mx-auto max-w-7xl px-4 py-8 sm:px-6 sm:py-12">
          <h3 className="text-lg font-semibold text-zinc-100 mb-6">
            Feature-by-feature comparison
          </h3>
          <div className="rounded-xl border border-zinc-800 bg-zinc-900/30 overflow-hidden">
            <FeatureTable rows={featureComparison} />
          </div>
          <p className="mt-4 text-xs text-zinc-600">
            lnget excels as a quick CLI tool for one-off requests. bolt402 is
            built for production code, AI agents, and multi-language SDKs.
          </p>
        </div>
      </section>

      {/* When to use which */}
      <section className="border-b border-zinc-800">
        <div className="mx-auto max-w-7xl px-4 py-8 sm:px-6 sm:py-12">
          <h3 className="text-lg font-semibold text-zinc-100 mb-6">
            When to use which
          </h3>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <div className="rounded-xl border border-zinc-800 bg-zinc-900/30 p-5">
              <h4 className="text-sm font-semibold text-zinc-300 mb-3">
                Choose lnget when...
              </h4>
              <ul className="space-y-2 text-xs text-zinc-500">
                <li className="flex items-start gap-2">
                  <span className="text-zinc-400">→</span>
                  You need a quick CLI download or one-off API call
                </li>
                <li className="flex items-start gap-2">
                  <span className="text-zinc-400">→</span>
                  Shell scripts or CI pipelines
                </li>
                <li className="flex items-start gap-2">
                  <span className="text-zinc-400">→</span>
                  Testing L402 endpoints manually
                </li>
                <li className="flex items-start gap-2">
                  <span className="text-zinc-400">→</span>
                  You want wget/curl-like UX with L402 built in
                </li>
              </ul>
            </div>

            <div className="rounded-xl border border-[#F7931A]/30 bg-zinc-900/30 p-5">
              <h4 className="text-sm font-semibold text-[#F7931A] mb-3">
                Choose bolt402 when...
              </h4>
              <ul className="space-y-2 text-xs text-zinc-500">
                <li className="flex items-start gap-2">
                  <span className="text-[#F7931A]">→</span>
                  Building an application that consumes L402 APIs
                </li>
                <li className="flex items-start gap-2">
                  <span className="text-[#F7931A]">→</span>
                  AI agents that need autonomous Lightning payments
                </li>
                <li className="flex items-start gap-2">
                  <span className="text-[#F7931A]">→</span>
                  You need typed errors, budget control, or receipt tracking
                </li>
                <li className="flex items-start gap-2">
                  <span className="text-[#F7931A]">→</span>
                  Multi-language projects (Rust, TypeScript, Python)
                </li>
                <li className="flex items-start gap-2">
                  <span className="text-[#F7931A]">→</span>
                  Production systems where subprocess overhead matters
                </li>
              </ul>
            </div>
          </div>
        </div>
      </section>

      {/* Footer */}
      <footer className="border-t border-zinc-800 bg-zinc-950">
        <div className="mx-auto max-w-7xl px-4 py-4 sm:px-6">
          <p className="text-center text-[10px] text-zinc-600">
            bolt402 vs lnget comparison — by{" "}
            <a
              href="https://github.com/lightninglabs/bolt402"
              target="_blank"
              rel="noopener noreferrer"
              className="text-zinc-500 hover:text-[#F7931A] transition-colors"
            >
              bolt402
            </a>
            . Both are open source. Both advance the L402 ecosystem.
          </p>
        </div>
      </footer>
    </div>
  );
}
