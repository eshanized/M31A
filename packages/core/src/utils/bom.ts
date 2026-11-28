/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

export const UTF8_BOM = '\uFEFF';

export interface SplitBomResult {
  bom: boolean;
  text: string;
}

/**
 * Strips any leading UTF-8 BOM (`\uFEFF`) and records whether one was present.
 */
export function splitBom(text: string): SplitBomResult {
  const stripped = text.replace(/^\uFEFF+/, '');
  return {
    bom: stripped.length !== text.length,
    text: stripped,
  };
}

/**
 * Re-attaches at most one leading UTF-8 BOM when `bom` is true.
 */
export function joinBom(text: string, bom: boolean): string {
  const stripped = splitBom(text).text;
  return bom ? UTF8_BOM + stripped : stripped;
}

/**
 * Normalizes curly typography (smart quotes, typographic dashes, and Unicode
 * non-breaking/wide spaces) into plain ASCII equivalents for tolerant matching.
 */
export function normalizeSmartUnicode(value: string): string {
  return value
    .replace(/[‘’‚‛]/g, "'")
    .replace(/[“”„‟]/g, '"')
    .replace(/[‐‑‒–—―−]/g, '-')
    .replace(/[\u00A0\u2002-\u200A\u202F\u205F\u3000]/g, ' ');
}
