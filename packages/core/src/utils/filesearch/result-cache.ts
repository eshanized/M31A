/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

/**
 * Implements an in-memory cache for file search results.
 * This cache optimizes subsequent searches by leveraging previously computed results.
 */
export class ResultCache {
  private readonly cache: Map<string, string[]>;

  constructor(private readonly allFiles: string[]) {
    this.cache = new Map();
  }

  /**
   * Retrieves cached search results for a given query, or provides a base set
   * of files to search from.
   * @param query The search query pattern.
   * @returns An object containing the files to search and a boolean indicating
   *          if the result is an exact cache hit.
   */
  get(query: string): { files: string[]; isExactMatch: boolean } {
    const exactMatch = this.cache.get(query);
    if (exactMatch) {
      return { files: exactMatch, isExactMatch: true };
    }

    // If a user first searches for "foo", and then for "foobar",
    // start from the cached results of "foo" instead of scanning all files.
    let bestBaseQuery = '';
    for (const key of this.cache.keys()) {
      if (query.startsWith(key) && key.length > bestBaseQuery.length) {
        bestBaseQuery = key;
      }
    }

    const filesToSearch = bestBaseQuery
      ? this.cache.get(bestBaseQuery)!
      : this.allFiles;

    return { files: filesToSearch, isExactMatch: false };
  }

  /**
   * Stores search results in the cache.
   * @param query The search query pattern.
   * @param results The matching file paths to cache.
   */
  set(query: string, results: string[]): void {
    this.cache.set(query, results);
  }
}
