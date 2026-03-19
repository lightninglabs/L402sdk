'use client';

import type { L402Service } from '@/lib/types';

interface ServiceCardProps {
  service: L402Service;
  onSelect: (service: L402Service) => void;
}

const healthColor: Record<string, string> = {
  healthy: 'bg-emerald-500',
  degraded: 'bg-yellow-500',
  down: 'bg-red-500',
  unknown: 'bg-zinc-500',
};

export default function ServiceCard({ service, onSelect }: ServiceCardProps) {
  const truncatedDesc =
    service.description && service.description.length > 120
      ? service.description.slice(0, 120) + '…'
      : service.description || '';

  return (
    <button
      onClick={() => onSelect(service)}
      className="group relative flex flex-col gap-3 rounded-xl border border-zinc-800 bg-zinc-900/50 p-5 text-left transition-all hover:border-[#F7931A]/40 hover:bg-zinc-900 hover:shadow-[0_0_24px_rgba(247,147,26,0.06)] focus:outline-none focus:ring-2 focus:ring-[#F7931A]/50"
    >
      {/* Header */}
      <div className="flex items-start justify-between gap-2">
        <h3 className="text-sm font-semibold text-zinc-100 group-hover:text-[#F7931A] transition-colors leading-tight">
          {service.name}
        </h3>
        <div className="flex items-center gap-1.5 shrink-0">
          <span
            title={service.health_status}
            className={`h-2 w-2 rounded-full ${healthColor[service.health_status] || 'bg-zinc-500'}`}
          />
          <span className="rounded-full bg-[#F7931A]/10 px-2 py-0.5 text-[10px] font-medium text-[#F7931A] uppercase tracking-wider">
            {service.protocol}
          </span>
        </div>
      </div>

      {/* Description */}
      <p className="text-xs leading-relaxed text-zinc-400 flex-1">
        {truncatedDesc}
      </p>

      {/* Footer */}
      <div className="flex items-center justify-between pt-1">
        {/* Category */}
        <span className="rounded-md bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-400">
          {service.category}
        </span>

        {/* Pricing */}
        <div className="flex items-center gap-1 text-xs font-mono">
          <span className="text-[#F7931A]">⚡</span>
          {service.price_sats != null ? (
            <span className="text-zinc-300">{service.price_sats} sats</span>
          ) : service.price_usd != null ? (
            <span className="text-zinc-300">${service.price_usd}</span>
          ) : (
            <span className="text-zinc-500">price unknown</span>
          )}
        </div>
      </div>

      {/* Meta row */}
      <div className="flex items-center justify-between text-[10px] text-zinc-600">
        <span>{service.provider || 'Unknown provider'}</span>
        <div className="flex items-center gap-2">
          {service.uptime_30d != null && (
            <span>{(service.uptime_30d * 100).toFixed(0)}% uptime</span>
          )}
          {service.latency_p50_ms != null && (
            <span>{service.latency_p50_ms}ms</span>
          )}
        </div>
      </div>
    </button>
  );
}
