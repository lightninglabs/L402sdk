import type { L402Service, CategoryOption } from './types';

const INDEX_API = process.env.INDEX_API_URL || 'https://402index.io/api/v1';

/** Fetch L402 services from 402index.io. */
export async function fetchServices(
  category?: string,
  query?: string,
): Promise<L402Service[]> {
  const params = new URLSearchParams();
  params.set('protocol', 'l402');
  params.set('limit', '100');
  if (category && category !== 'all') params.set('category', category);
  if (query) params.set('q', query);

  const url = `${INDEX_API}/services?${params}`;

  const res = await fetch(url, { next: { revalidate: 300 } });
  if (!res.ok) throw new Error(`402index API error: ${res.status}`);

  const data = await res.json();
  return data.services ?? [];
}

/** Extract categories from fetched services (so counts match the displayed grid). */
export function extractCategories(services: L402Service[]): CategoryOption[] {
  const counts = new Map<string, number>();

  for (const svc of services) {
    const cat = svc.category || 'uncategorized';
    // Use top-level category (before the slash) for grouping
    const topLevel = cat.split('/')[0];
    counts.set(topLevel, (counts.get(topLevel) || 0) + 1);
  }

  return Array.from(counts.entries())
    .map(([slug, count]) => ({ slug, name: slug, count }))
    .sort((a, b) => b.count - a.count);
}
