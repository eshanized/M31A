/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { tokenLimit } from '@m31a/core';

export function getContextUsagePercentage(
  promptTokenCount: number,
  model: string | undefined,
  contextWindowOverride?: number,
): number {
  if (!model || typeof model !== 'string' || model.length === 0) {
    return 0;
  }
  const limit = tokenLimit(model, contextWindowOverride);
  if (limit <= 0) {
    return 0;
  }
  return promptTokenCount / limit;
}

export function isContextUsageHigh(
  promptTokenCount: number,
  model: string | undefined,
  threshold = 0.6,
  contextWindowOverride?: number,
): boolean {
  return (
    getContextUsagePercentage(promptTokenCount, model, contextWindowOverride) >
    threshold
  );
}
