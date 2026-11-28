/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { estimateTokenCountSync } from './tokenCalculation.js';

export type ThoughtSummary = {
  subject: string;
  description: string;
};

const START_DELIMITER = '**';
const END_DELIMITER = '**';

/**
 * Parses a raw thought string into a structured ThoughtSummary object.
 *
 * Thoughts are expected to have a bold "subject" part enclosed in double
 * asterisks (e.g., **Subject**). The rest of the string is considered
 * the description. This function only parses the first valid subject found.
 *
 * @param rawText The raw text of the thought.
 * @returns A ThoughtSummary object. If no valid subject is found, the entire
 * string is treated as the description.
 */
export function parseThought(rawText: string): ThoughtSummary {
  const startIndex = rawText.indexOf(START_DELIMITER);
  if (startIndex === -1) {
    // No start delimiter found, the whole text is the description.
    return { subject: '', description: rawText.trim() };
  }

  const endIndex = rawText.indexOf(
    END_DELIMITER,
    startIndex + START_DELIMITER.length,
  );
  if (endIndex === -1) {
    // Start delimiter found but no end delimiter, so it's not a valid subject.
    // Treat the entire string as the description.
    return { subject: '', description: rawText.trim() };
  }

  const subject = rawText
    .substring(startIndex + START_DELIMITER.length, endIndex)
    .trim();

  // The description is everything before the start delimiter and after the end delimiter.
  const description = (
    rawText.substring(0, startIndex) +
    rawText.substring(endIndex + END_DELIMITER.length)
  ).trim();

  return { subject, description };
}

function joinThoughtDescription(prev: string, next: string): string {
  if (!prev) return next;
  if (!next) return prev;
  const needsSpace = !/\s$/.test(prev) && !/^[\s.,;:!?)'\]\n]/.test(next);
  return `${prev}${needsSpace ? ' ' : ''}${next}`;
}

/**
 * Appends a ThoughtSummary to an existing list, coalescing consecutive chunks
 * that continue the current thought (empty subject or identical subject).
 */
export function appendThoughtSummary(
  entries: ThoughtSummary[],
  next: ThoughtSummary,
): ThoughtSummary[] {
  const { subject, description } = next;
  if (entries.length === 0) {
    return [{ subject, description }];
  }

  const last = entries[entries.length - 1];
  const isContinuation =
    subject === '' || (subject === last.subject && last.subject !== '');
  if (!isContinuation) {
    return [...entries, { subject, description }];
  }

  const updatedLast: ThoughtSummary = {
    subject: last.subject,
    description: joinThoughtDescription(last.description, description),
  };
  return [...entries.slice(0, -1), updatedLast];
}

/**
 * Estimates the token count of a ThoughtSummary from its text content.
 */
export function estimateThoughtTokens(thought: ThoughtSummary): number {
  const text = [thought.subject, thought.description]
    .map((part) => part.trim())
    .filter(Boolean)
    .join('\n');
  if (!text) {
    return 0;
  }
  return Math.max(1, estimateTokenCountSync([{ text }]));
}

/**
 * Allocates token counts across thought entries, preferring a positive
 * API-reported total and falling back to heuristic text estimation.
 */
export function allocateThoughtTokenCounts(
  thoughts: readonly ThoughtSummary[],
  totalThoughtsTokens?: number,
): number[] {
  if (thoughts.length === 0) {
    return [];
  }

  if (totalThoughtsTokens !== undefined && totalThoughtsTokens > 0) {
    if (thoughts.length === 1) {
      return [totalThoughtsTokens];
    }

    const weights = thoughts.map((t) =>
      Math.max(1, t.subject.trim().length + t.description.trim().length),
    );
    const totalWeight = weights.reduce((sum, w) => sum + w, 0);
    let remaining = totalThoughtsTokens;
    return weights.map((weight, index) => {
      if (index === weights.length - 1) {
        return Math.max(0, remaining);
      }
      const share = Math.round((totalThoughtsTokens * weight) / totalWeight);
      const clamped = Math.min(remaining, Math.max(0, share));
      remaining -= clamped;
      return clamped;
    });
  }

  return thoughts.map((thought) => estimateThoughtTokens(thought));
}
