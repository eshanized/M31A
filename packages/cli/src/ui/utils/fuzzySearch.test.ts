/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, expect, it } from 'vitest';
import { fuzzyFilter, fuzzyScore } from './fuzzySearch.js';

describe('fuzzySearch', () => {
  it('returns 0 when query is empty and -1 when characters cannot match in order', () => {
    expect(fuzzyScore('', 'settings')).toBe(0);
    expect(fuzzyScore('xyz', 'settings')).toBe(-1);
    expect(fuzzyScore('gn', 'settings')).toBe(-1);
  });

  it('ranks exact and prefix matches above loose subsequence matches', () => {
    const exact = fuzzyScore('theme', 'theme');
    const prefix = fuzzyScore('thm', 'theme');
    const subsequence = fuzzyScore('tm', 'set time mode');

    expect(exact).toBeGreaterThan(prefix);
    expect(prefix).toBeGreaterThan(subsequence);
  });

  it('filters and sorts items across primary and secondary fields', () => {
    const items = [
      { name: 'clear', description: 'Clear screen and reset UI' },
      { name: 'theme', description: 'Switch terminal color theme' },
      { name: 'model', description: 'Select NVIDIA NIM model' },
    ];

    const matched = fuzzyFilter(items, 'thm', (item) => [
      item.name,
      item.description,
    ]);
    expect(matched.map((entry) => entry.item.name)).toEqual(['theme']);

    const emptyQuery = fuzzyFilter(items, '  ', (item) => [item.name]);
    expect(emptyQuery).toHaveLength(3);
  });
});
