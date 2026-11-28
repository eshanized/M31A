/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import crypto from 'node:crypto';

interface CacheEntry {
  results: string[];
  expiresAt: number;
}

const crawlCache = new Map<string, CacheEntry>();

/**
 * Generates a unique cache key based on the project directory and the content
 * of ignore files. This ensures that the cache is invalidated if the project
 * or ignore rules change.
 */
export const getCacheKey = (
  directory: string,
  ignoreContent: string,
  maxDepth?: number,
): string => {
  const hash = crypto.createHash('sha256');
  hash.update(directory);
  hash.update(ignoreContent);
  if (maxDepth !== undefined) {
    hash.update(String(maxDepth));
  }
  return hash.digest('hex');
};

/**
 * Reads cached data from the in-memory cache.
 * Returns undefined if the key is not found.
 */
export const read = (key: string): string[] | undefined => {
  const entry = crawlCache.get(key);
  if (!entry) {
    return undefined;
  }
  if (Date.now() >= entry.expiresAt) {
    crawlCache.delete(key);
    return undefined;
  }
  return entry.results;
};

/**
 * Writes data to the in-memory cache with an expiration timestamp.
 */
export const write = (key: string, results: string[], ttlMs: number): void => {
  crawlCache.set(key, {
    results,
    expiresAt: Date.now() + ttlMs,
  });
};

/**
 * Clears the entire cache.
 * Primarily used for testing.
 */
export const clear = (): void => {
  crawlCache.clear();
};
