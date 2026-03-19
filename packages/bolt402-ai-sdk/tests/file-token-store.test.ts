import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { randomBytes } from 'node:crypto';
import { FileTokenStore } from '../src/file-token-store.js';

describe('FileTokenStore', () => {
  let testDir: string;
  let filePath: string;

  beforeEach(() => {
    testDir = join(tmpdir(), `bolt402-test-${randomBytes(4).toString('hex')}`);
    mkdirSync(testDir, { recursive: true });
    filePath = join(testDir, 'tokens.json');
  });

  afterEach(() => {
    try {
      rmSync(testDir, { recursive: true, force: true });
    } catch {
      // Cleanup best-effort
    }
  });

  it('returns null for missing endpoint', async () => {
    const store = new FileTokenStore(filePath);
    const result = await store.get('https://api.example.com/data');
    expect(result).toBeNull();
  });

  it('stores and retrieves a token', async () => {
    const store = new FileTokenStore(filePath);
    await store.put('https://api.example.com/data', 'mac1', 'pre1');

    const result = await store.get('https://api.example.com/data');
    expect(result).toEqual({ macaroon: 'mac1', preimage: 'pre1' });
  });

  it('persists tokens to disk', async () => {
    const store1 = new FileTokenStore(filePath);
    await store1.put('https://api.example.com/data', 'mac1', 'pre1');

    // Create a new store instance (simulates process restart)
    const store2 = new FileTokenStore(filePath);
    const result = await store2.get('https://api.example.com/data');
    expect(result).toEqual({ macaroon: 'mac1', preimage: 'pre1' });
  });

  it('overwrites existing token', async () => {
    const store = new FileTokenStore(filePath);
    await store.put('https://api.example.com/data', 'mac1', 'pre1');
    await store.put('https://api.example.com/data', 'mac2', 'pre2');

    const result = await store.get('https://api.example.com/data');
    expect(result).toEqual({ macaroon: 'mac2', preimage: 'pre2' });
  });

  it('removes a token', async () => {
    const store = new FileTokenStore(filePath);
    await store.put('https://api.example.com/data', 'mac1', 'pre1');
    await store.remove('https://api.example.com/data');

    const result = await store.get('https://api.example.com/data');
    expect(result).toBeNull();
  });

  it('clears all tokens', async () => {
    const store = new FileTokenStore(filePath);
    await store.put('https://a.com', 'mac1', 'pre1');
    await store.put('https://b.com', 'mac2', 'pre2');

    await store.clear();

    expect(store.size).toBe(0);
    expect(await store.get('https://a.com')).toBeNull();
  });

  it('reports correct size', async () => {
    const store = new FileTokenStore(filePath);
    expect(store.size).toBe(0);

    await store.put('https://a.com', 'mac1', 'pre1');
    expect(store.size).toBe(1);

    await store.put('https://b.com', 'mac2', 'pre2');
    expect(store.size).toBe(2);

    await store.remove('https://a.com');
    expect(store.size).toBe(1);
  });

  it('handles corrupted file gracefully', async () => {
    writeFileSync(filePath, 'not valid json{{{', 'utf-8');

    const store = new FileTokenStore(filePath);
    const result = await store.get('https://api.com');
    expect(result).toBeNull();
    expect(store.size).toBe(0);
  });

  it('creates parent directories if needed', async () => {
    const deepPath = join(testDir, 'deep', 'nested', 'tokens.json');

    const store = new FileTokenStore(deepPath);
    await store.put('https://api.com', 'mac', 'pre');

    expect(existsSync(deepPath)).toBe(true);
  });

  it('exposes the file path', () => {
    const store = new FileTokenStore(filePath);
    expect(store.path).toBe(filePath);
  });

  it('handles multiple endpoints', async () => {
    const store = new FileTokenStore(filePath);
    await store.put('https://a.com/1', 'mac1', 'pre1');
    await store.put('https://b.com/2', 'mac2', 'pre2');
    await store.put('https://c.com/3', 'mac3', 'pre3');

    expect(await store.get('https://a.com/1')).toEqual({ macaroon: 'mac1', preimage: 'pre1' });
    expect(await store.get('https://b.com/2')).toEqual({ macaroon: 'mac2', preimage: 'pre2' });
    expect(await store.get('https://c.com/3')).toEqual({ macaroon: 'mac3', preimage: 'pre3' });
  });
});
