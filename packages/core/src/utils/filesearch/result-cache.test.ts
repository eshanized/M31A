/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { test, expect } from 'vitest';
import { ResultCache } from './result-cache.js';

test('ResultCache basic usage', () => {
  const files = [
    'foo.txt',
    'bar.js',
    'baz.md',
    'subdir/file.txt',
    'subdir/other.js',
    'subdir/nested/file.md',
  ];
  const cache = new ResultCache(files);
  const { files: resultFiles, isExactMatch } = cache.get('*.js');
  expect(resultFiles).toEqual(files);
  expect(isExactMatch).toBe(false);
});

test('ResultCache cache hit/miss', () => {
  const files = ['foo.txt', 'bar.js', 'baz.md'];
  const cache = new ResultCache(files);
  // First call: miss
  const { files: result1Files, isExactMatch: isExactMatch1 } =
    cache.get('*.js');
  expect(result1Files).toEqual(files);
  expect(isExactMatch1).toBe(false);

  // Simulate FileSearch applying the filter and setting the result
  cache.set('*.js', ['bar.js']);

  // Second call: hit
  const { files: result2Files, isExactMatch: isExactMatch2 } =
    cache.get('*.js');
  expect(result2Files).toEqual(['bar.js']);
  expect(isExactMatch2).toBe(true);
});

test('ResultCache best base query', () => {
  const files = ['foo.txt', 'foobar.js', 'baz.md'];
  const cache = new ResultCache(files);

  // Cache a broader query
  cache.set('foo', ['foo.txt', 'foobar.js']);

  // Search for a more specific query that starts with the broader one
  const { files: resultFiles, isExactMatch } = cache.get('foobar');
  expect(resultFiles).toEqual(['foo.txt', 'foobar.js']);
  expect(isExactMatch).toBe(false);
});
