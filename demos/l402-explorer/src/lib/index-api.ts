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

/** Fetch categories from 402index.io with L402 counts. */
export async function fetchCategories(): Promise<CategoryOption[]> {
  const res = await fetch(`${INDEX_API}/categories`, { next: { revalidate: 600 } });
  if (!res.ok) throw new Error(`402index categories error: ${res.status}`);

  const data = await res.json();
  const categories: CategoryOption[] = [];

  for (const [slug, counts] of Object.entries(data.categories)) {
    const l402Count = (counts as Record<string, number>).L402 ?? 0;
    if (l402Count > 0) {
      categories.push({ slug, name: slug, count: l402Count });
    }
  }

  return categories.sort((a, b) => b.count - a.count);
}
