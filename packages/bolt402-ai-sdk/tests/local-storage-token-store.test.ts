import { describe, it, expect, beforeEach } from 'vitest';
import { LocalStorageTokenStore } from '../src/local-storage-token-store.js';

/** Minimal Storage mock for testing. */
class MockStorage implements Storage {
  private store = new Map<string, string>();

  get length(): number {
    return this.store.size;
  }

  clear(): void {
    this.store.clear();
  }

  getItem(key: string): string | null {
    return this.store.get(key) ?? null;
  }

  key(index: number): string | null {
    const keys = [...this.store.keys()];
    return keys[index] ?? null;
  }

  removeItem(key: string): void {
    this.store.delete(key);
  }

  setItem(key: string, value: string): void {
    this.store.set(key, value);
  }
}

describe('LocalStorageTokenStore', () => {
  let storage: MockStorage;
  let store: LocalStorageTokenStore;

  beforeEach(() => {
    storage = new MockStorage();
    store = new LocalStorageTokenStore('bolt402:token:', storage);
  });

  it('returns null for missing endpoint', async () => {
    const result = await store.get('https://api.example.com/data');
    expect(result).toBeNull();
  });

  it('stores and retrieves a token', async () => {
    await store.put('https://api.example.com/data', 'mac1', 'pre1');

    const result = await store.get('https://api.example.com/data');
    expect(result).toEqual({ macaroon: 'mac1', preimage: 'pre1' });
  });

  it('overwrites existing token', async () => {
    await store.put('https://api.example.com/data', 'mac1', 'pre1');
    await store.put('https://api.example.com/data', 'mac2', 'pre2');

    const result = await store.get('https://api.example.com/data');
    expect(result).toEqual({ macaroon: 'mac2', preimage: 'pre2' });
  });

  it('removes a token', async () => {
    await store.put('https://api.example.com/data', 'mac1', 'pre1');
    await store.remove('https://api.example.com/data');

    const result = await store.get('https://api.example.com/data');
    expect(result).toBeNull();
  });

  it('clears all tokens with matching prefix', async () => {
    await store.put('https://api.example.com/a', 'mac1', 'pre1');
    await store.put('https://api.example.com/b', 'mac2', 'pre2');

    // Add a non-bolt402 entry
    storage.setItem('other:key', 'value');

    await store.clear();

    expect(store.size).toBe(0);
    // Non-bolt402 entry should survive
    expect(storage.getItem('other:key')).toBe('value');
  });

  it('reports correct size', async () => {
    expect(store.size).toBe(0);

    await store.put('https://a.com', 'mac1', 'pre1');
    expect(store.size).toBe(1);

    await store.put('https://b.com', 'mac2', 'pre2');
    expect(store.size).toBe(2);

    await store.remove('https://a.com');
    expect(store.size).toBe(1);
  });

  it('handles corrupted JSON gracefully', async () => {
    storage.setItem('bolt402:token:https://bad.com', 'not valid json{{{');

    const result = await store.get('https://bad.com');
    expect(result).toBeNull();

    // Corrupted entry should be removed
    expect(storage.getItem('bolt402:token:https://bad.com')).toBeNull();
  });

  it('handles missing fields gracefully', async () => {
    storage.setItem('bolt402:token:https://partial.com', JSON.stringify({ macaroon: 'mac' }));

    const result = await store.get('https://partial.com');
    expect(result).toBeNull();
  });

  it('uses custom prefix', async () => {
    const customStore = new LocalStorageTokenStore('myapp:', storage);
    await customStore.put('https://api.com', 'mac', 'pre');

    expect(storage.getItem('myapp:https://api.com')).toBeTruthy();
    expect(storage.getItem('bolt402:token:https://api.com')).toBeNull();
  });

  it('degrades gracefully when storage is null', async () => {
    // Create store without storage (simulates SSR/unavailable localStorage)
    const noStorageStore = new LocalStorageTokenStore('bolt402:token:', undefined as unknown as Storage);

    // These should all be no-ops, not throw
    await noStorageStore.put('https://api.com', 'mac', 'pre');
    const result = await noStorageStore.get('https://api.com');
    expect(result).toBeNull();
    expect(noStorageStore.size).toBe(0);
    await noStorageStore.remove('https://api.com');
    await noStorageStore.clear();
  });
});
