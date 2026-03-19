/**
 * File-backed token store for Node.js environments.
 *
 * Persists L402 tokens to a JSON file on disk so they survive process
 * restarts. Uses atomic writes (write to temp file + rename) to prevent
 * corruption. Similar to how lnget stores tokens at `~/.lnget/tokens/`.
 *
 * @example
 * ```typescript
 * import { FileTokenStore } from 'bolt402-ai-sdk';
 *
 * // Default path: ~/.bolt402/tokens.json
 * const store = new FileTokenStore();
 *
 * // Custom path:
 * const store = new FileTokenStore('/tmp/my-tokens.json');
 * ```
 */

import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { homedir } from 'node:os';
import { randomBytes } from 'node:crypto';

import type { CachedToken, TokenStore } from './types.js';

/** Default token file path. */
const DEFAULT_PATH = join(homedir(), '.bolt402', 'tokens.json');

/** Internal representation of the tokens file. */
interface TokensFile {
  version: 1;
  tokens: Record<string, CachedToken>;
}

/** Token store that persists to a JSON file on disk. */
export class FileTokenStore implements TokenStore {
  private readonly filePath: string;
  private cache: Map<string, CachedToken>;
  private loaded: boolean;

  /**
   * Create a new file-backed token store.
   *
   * @param filePath Path to the JSON file. Default: `~/.bolt402/tokens.json`.
   */
  constructor(filePath: string = DEFAULT_PATH) {
    this.filePath = filePath;
    this.cache = new Map();
    this.loaded = false;
  }

  async get(endpoint: string): Promise<CachedToken | null> {
    this.ensureLoaded();
    return this.cache.get(endpoint) ?? null;
  }

  async put(endpoint: string, macaroon: string, preimage: string): Promise<void> {
    this.ensureLoaded();
    this.cache.set(endpoint, { macaroon, preimage });
    this.flush();
  }

  async remove(endpoint: string): Promise<void> {
    this.ensureLoaded();
    this.cache.delete(endpoint);
    this.flush();
  }

  async clear(): Promise<void> {
    this.cache.clear();
    this.loaded = true;
    this.flush();
  }

  /** Get the current number of cached tokens. */
  get size(): number {
    this.ensureLoaded();
    return this.cache.size;
  }

  /** Get the file path used by this store. */
  get path(): string {
    return this.filePath;
  }

  /** Load tokens from disk if not already loaded. */
  private ensureLoaded(): void {
    if (this.loaded) return;
    this.loaded = true;

    try {
      if (!existsSync(this.filePath)) return;

      const raw = readFileSync(this.filePath, 'utf-8');
      const data = JSON.parse(raw) as TokensFile;

      if (data.version === 1 && data.tokens) {
        for (const [key, value] of Object.entries(data.tokens)) {
          if (value.macaroon && value.preimage) {
            this.cache.set(key, value);
          }
        }
      }
    } catch {
      // Corrupted file — start fresh
      this.cache.clear();
    }
  }

  /** Write tokens to disk atomically. */
  private flush(): void {
    const dir = dirname(this.filePath);

    try {
      if (!existsSync(dir)) {
        mkdirSync(dir, { recursive: true });
      }

      const data: TokensFile = {
        version: 1,
        tokens: Object.fromEntries(this.cache),
      };

      const json = JSON.stringify(data, null, 2) + '\n';

      // Atomic write: write to temp file in same directory, then rename
      const tmpPath = join(dir, `.tokens.${randomBytes(4).toString('hex')}.tmp`);
      writeFileSync(tmpPath, json, 'utf-8');
      renameSync(tmpPath, this.filePath);
    } catch {
      // If we can't write, degrade silently (in-memory only)
    }
  }
}
