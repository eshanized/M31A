/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

export interface FuzzyMatchResult<T> {
  item: T;
  score: number;
}

const WORD_BOUNDARY_RE = /[\s/_.:-]/;

/**
 * Computes a deterministic subsequence fuzzy score (higher is better, -1 = no match).
 * Rewards prefix matches, word-boundary hits, consecutive matches, and camelCase transitions.
 */
export function fuzzyScore(query: string, target: string): number {
  const cleanQuery = query.trim().toLowerCase();
  if (!cleanQuery) {
    return 0;
  }
  const cleanTarget = target.trim();
  if (!cleanTarget) {
    return -1;
  }

  const lowerTarget = cleanTarget.toLowerCase();
  if (lowerTarget === cleanQuery) {
    return 1000;
  }

  const exactIndex = lowerTarget.indexOf(cleanQuery);
  let qi = 0;
  let score = 0;
  let consecutive = 0;

  for (let ti = 0; ti < cleanTarget.length && qi < cleanQuery.length; ti++) {
    if (lowerTarget[ti] !== cleanQuery[qi]) {
      consecutive = 0;
      continue;
    }

    let charScore = 1;
    if (ti === 0) {
      charScore += 12;
    } else {
      const prevChar = cleanTarget[ti - 1] ?? '';
      const currChar = cleanTarget[ti] ?? '';
      if (WORD_BOUNDARY_RE.test(prevChar)) {
        charScore += 10;
      } else if (
        prevChar >= 'a' &&
        prevChar <= 'z' &&
        currChar >= 'A' &&
        currChar <= 'Z'
      ) {
        charScore += 8;
      }
    }

    consecutive += 1;
    charScore += Math.min(consecutive, 5) * 4;
    score += charScore;
    qi += 1;
  }

  if (qi < cleanQuery.length) {
    return -1;
  }

  if (exactIndex === 0) {
    score += 40;
  } else if (exactIndex > 0) {
    score += 15;
  }

  // Slight tie-breaker favoring tighter/shorter targets.
  score -= Math.min(
    20,
    Math.floor((cleanTarget.length - cleanQuery.length) / 4),
  );
  return Math.max(1, score);
}

/**
 * Filters and ranks items against a query using one or more text selectors.
 */
export function fuzzyFilter<T>(
  items: readonly T[],
  query: string,
  getTextFields: (item: T) => readonly string[],
): Array<FuzzyMatchResult<T>> {
  const cleanQuery = query.trim();
  if (!cleanQuery) {
    return items.map((item) => ({ item, score: 0 }));
  }

  const matches: Array<FuzzyMatchResult<T>> = [];
  for (const item of items) {
    let bestScore = -1;
    const fields = getTextFields(item);
    for (let i = 0; i < fields.length; i++) {
      const candidate = fields[i];
      if (!candidate) continue;
      const fieldScore = fuzzyScore(cleanQuery, candidate);
      if (fieldScore >= 0) {
        // Prefer primary (label) match over secondary (description) match.
        const weighted = i === 0 ? fieldScore * 2 : fieldScore;
        if (weighted > bestScore) {
          bestScore = weighted;
        }
      }
    }
    if (bestScore >= 0) {
      matches.push({ item, score: bestScore });
    }
  }

  return matches.sort((a, b) => b.score - a.score);
}
