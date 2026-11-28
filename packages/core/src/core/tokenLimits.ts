/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import {
  DEFAULT_M31A_MODEL,
  M31A_MODEL_ALIAS_AUTO,
  M31A_MODEL_ALIAS_FLASH,
  M31A_MODEL_ALIAS_FLASH_LITE,
  M31A_MODEL_ALIAS_PRO,
} from '../config/models.js';
import { getNimModelTokenLimits } from './nvidiaNimProtocolHelpers.js';

type Model = string;
type TokenCount = number;

export const DEFAULT_TOKEN_LIMIT = 1_048_576;
export const DEFAULT_OUTPUT_TOKEN_LIMIT = 65_536;

export function tokenLimit(
  model: Model,
  overrideContextWindow?: number,
): TokenCount {
  if (
    typeof overrideContextWindow === 'number' &&
    Number.isFinite(overrideContextWindow) &&
    overrideContextWindow > 0
  ) {
    return Math.floor(overrideContextWindow);
  }
  if (typeof model === 'string') {
    const nimLimits = getNimModelTokenLimits(model);
    if (nimLimits) {
      return nimLimits.context;
    }
  }
  return DEFAULT_TOKEN_LIMIT;
}

export function outputTokenLimit(model: Model): TokenCount | undefined {
  switch (model) {
    case DEFAULT_M31A_MODEL:
    case M31A_MODEL_ALIAS_AUTO:
    case M31A_MODEL_ALIAS_PRO:
    case M31A_MODEL_ALIAS_FLASH:
    case M31A_MODEL_ALIAS_FLASH_LITE:
      return DEFAULT_OUTPUT_TOKEN_LIMIT;
    default:
      return typeof model === 'string'
        ? getNimModelTokenLimits(model)?.output
        : undefined;
  }
}
