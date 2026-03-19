/**
 * LocalStorage-backed token store for browser environments.
 *
 * Persists L402 tokens in the browser's `localStorage` so they survive
 * page reloads and browser restarts. Falls back gracefully when
 * localStorage is unavailable (e.g., SSR, incognito quota exceeded).
 *
 * @example
 * ```typescript
 * import { LocalStorageTokenStore } from 'bolt402-ai-sdk';
 *
 * const store = new LocalStorageTokenStore();
 * // or with a custom prefix:
 * const store = new LocalStorageTokenStore('myapp:l402:');
 * ```
 */

import type { CachedToken, TokenStore } from './types.js';

/** Default key prefix for localStorage entries. */
const DEFAULT_PREFIX = 'bolt402:token:';

/** Token store that persists to browser localStorage. */
export class LocalStorageTokenStore implements TokenStore {
  private readonly prefix: string;
  private readonly storage: Storage | null;

  /**
   * Create a new LocalStorage-backed token store.
   *
   * @param prefix Key prefix for all stored tokens. Default: `bolt402:token:`.
   * @param storage Custom Storage implementation (for testing). Defaults to `window.localStorage`.
   */
  constructor(prefix: string = DEFAULT_PREFIX, storage?: Storage) {
    this.prefix = prefix;
    this.storage = storage ?? LocalStorageTokenStore.detectStorage();
  }

  async get(endpoint: string): Promise<CachedToken | null> {
    if (!this.storage) return null;

    try {
      const raw = this.storage.getItem(this.key(endpoint));
      if (!raw) return null;

      const parsed = JSON.parse(raw) as CachedToken;
      if (!parsed.macaroon || !parsed.preimage) return null;

      return parsed;
    } catch {
      // Corrupted entry — remove it
      this.storage.removeItem(this.key(endpoint));
      return null;
    }
  }

  async put(endpoint: string, macaroon: string, preimage: string): Promise<void> {
    if (!this.storage) return;

    const token: CachedToken = { macaroon, preimage };

    try {
      this.storage.setItem(this.key(endpoint), JSON.stringify(token));
    } catch {
      // Quota exceeded — try to clear old bolt402 entries and retry once
      this.evictOldest();
      try {
        this.storage.setItem(this.key(endpoint), JSON.stringify(token));
      } catch {
        // Still failing — silently degrade to in-memory behavior
      }
    }
  }

  async remove(endpoint: string): Promise<void> {
    if (!this.storage) return;
    this.storage.removeItem(this.key(endpoint));
  }

  async clear(): Promise<void> {
    if (!this.storage) return;

    const keysToRemove: string[] = [];
    for (let i = 0; i < this.storage.length; i++) {
      const key = this.storage.key(i);
      if (key?.startsWith(this.prefix)) {
        keysToRemove.push(key);
      }
    }

    for (const key of keysToRemove) {
      this.storage.removeItem(key);
    }
  }

  /** Get the number of stored tokens. */
  get size(): number {
    if (!this.storage) return 0;

    let count = 0;
    for (let i = 0; i < this.storage.length; i++) {
      const key = this.storage.key(i);
      if (key?.startsWith(this.prefix)) {
        count++;
      }
    }
    return count;
  }

  /** Build the full localStorage key for an endpoint. */
  private key(endpoint: string): string {
    return `${this.prefix}${endpoint}`;
  }

  /** Remove the first bolt402 entry found (simple eviction for quota issues). */
  private evictOldest(): void {
    if (!this.storage) return;

    for (let i = 0; i < this.storage.length; i++) {
      const key = this.storage.key(i);
      if (key?.startsWith(this.prefix)) {
        this.storage.removeItem(key);
        return;
      }
    }
  }

  /** Detect if localStorage is available. */
  private static detectStorage(): Storage | null {
    try {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      const win = globalThis as unknown as { localStorage?: Storage };
      if (win.localStorage) {
        // Test that it actually works (incognito Safari can throw)
        const testKey = '__bolt402_test__';
        win.localStorage.setItem(testKey, '1');
        win.localStorage.removeItem(testKey);
        return win.localStorage;
      }
    } catch {
      // localStorage not available
    }
    return null;
  }
}
