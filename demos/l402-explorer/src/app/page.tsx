import { fetchServices, extractCategories } from '@/lib/index-api';
import ServiceBrowser from '@/components/ServiceBrowser';

export default async function Home() {
  let services: Awaited<ReturnType<typeof fetchServices>> = [];
  try {
    services = await fetchServices();
  } catch {
    // Fall back to empty list if API is unreachable
  }

  const categories = extractCategories(services);

  return (
    <div className="flex flex-col min-h-screen">
      {/* Header */}
      <header className="sticky top-0 z-40 border-b border-zinc-800 bg-zinc-950/80 backdrop-blur-md">
        <div className="mx-auto flex max-w-7xl items-center justify-between px-4 py-3 sm:px-6">
          <div className="flex items-center gap-2.5">
            <span className="text-xl">⚡</span>
            <div>
              <h1 className="text-sm font-bold text-zinc-100 tracking-tight">
                L402 Explorer
              </h1>
              <p className="text-[10px] text-zinc-500">
                by bolt402
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
              href="https://402index.io"
              target="_blank"
              rel="noopener noreferrer"
              className="rounded-lg bg-[#F7931A]/10 px-3 py-1.5 text-xs font-medium text-[#F7931A] hover:bg-[#F7931A]/20 transition-colors"
            >
              402index.io
            </a>
          </div>
        </div>
      </header>

      {/* Hero */}
      <section className="border-b border-zinc-800 bg-zinc-950">
        <div className="mx-auto max-w-7xl px-4 py-10 sm:px-6 sm:py-14">
          <div className="max-w-2xl">
            <h2 className="text-2xl font-bold text-zinc-100 tracking-tight sm:text-3xl">
              Browse Lightning-payable APIs
            </h2>
            <p className="mt-2 text-sm leading-relaxed text-zinc-400">
              Explore the growing ecosystem of L402 services. Click any service to
              visualize the protocol flow: HTTP 402 challenge, Lightning invoice,
              payment, and authenticated access.
            </p>
            <div className="mt-4 flex items-center gap-4 text-xs text-zinc-500">
              <span className="flex items-center gap-1">
                <span className="h-1.5 w-1.5 rounded-full bg-emerald-500" />
                {services.length} L402 services
              </span>
              <span className="flex items-center gap-1">
                <span className="h-1.5 w-1.5 rounded-full bg-[#F7931A]" />
                Data from 402index.io
              </span>
            </div>
          </div>
        </div>
      </section>

      {/* Main content */}
      <main className="flex-1 mx-auto w-full max-w-7xl px-4 py-6 sm:px-6 sm:py-8">
        <ServiceBrowser initialServices={services} categories={categories} />
      </main>

      {/* Footer */}
      <footer className="border-t border-zinc-800 bg-zinc-950">
        <div className="mx-auto max-w-7xl px-4 py-4 sm:px-6">
          <p className="text-center text-[10px] text-zinc-600">
            L402 Explorer — an interactive demo by{' '}
            <a
              href="https://github.com/lightninglabs/bolt402"
              target="_blank"
              rel="noopener noreferrer"
              className="text-zinc-500 hover:text-[#F7931A] transition-colors"
            >
              bolt402
            </a>
            . Service data from{' '}
            <a
              href="https://402index.io"
              target="_blank"
              rel="noopener noreferrer"
              className="text-zinc-500 hover:text-[#F7931A] transition-colors"
            >
              402index.io
            </a>
            .
          </p>
        </div>
      </footer>
    </div>
  );
}
