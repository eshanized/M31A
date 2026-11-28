/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect } from 'vitest';
import {
  tokenLimit,
  outputTokenLimit,
  DEFAULT_TOKEN_LIMIT,
  DEFAULT_OUTPUT_TOKEN_LIMIT,
} from './tokenLimits.js';
import {
  DEFAULT_M31A_MODEL,
  DEFAULT_M31A_FLASH_LITE_MODEL,
  DEFAULT_M31A_FLASH_MODEL,
  M31A_MODEL_ALIAS_AUTO,
  M31A_MODEL_ALIAS_FLASH,
  M31A_MODEL_ALIAS_FLASH_LITE,
  M31A_MODEL_ALIAS_PRO,
} from '../config/models.js';

describe('tokenLimit', () => {
  it('should return the correct token limit for default models', () => {
    expect(tokenLimit(DEFAULT_M31A_MODEL)).toBe(1_048_576);
    expect(tokenLimit(DEFAULT_M31A_MODEL)).toBe(1_048_576);
    expect(tokenLimit(DEFAULT_M31A_FLASH_MODEL)).toBe(1_048_576);
    expect(tokenLimit(DEFAULT_M31A_FLASH_LITE_MODEL)).toBe(1_048_576);
  });

  it('should return catalog limits for known NVIDIA NIM models', () => {
    expect(tokenLimit('nvidia/nemotron-3.5-lightning-30b-a3b')).toBe(262_144);
    expect(tokenLimit('nvidia/nemotron-3-nano-omni-30b-a3b-reasoning')).toBe(
      256_000,
    );
    expect(tokenLimit('deepseek-ai/deepseek-v4.1-flash')).toBe(1_000_000);
  });

  it('should respect positive context window overrides when supplied', () => {
    expect(tokenLimit(DEFAULT_M31A_MODEL, 131_072)).toBe(131_072);
    expect(tokenLimit(DEFAULT_M31A_MODEL, 0)).toBe(1_048_576);
    expect(tokenLimit(DEFAULT_M31A_MODEL, -10)).toBe(1_048_576);
  });

  it('should return the default token limit for an unknown model', () => {
    expect(tokenLimit('unknown-model')).toBe(DEFAULT_TOKEN_LIMIT);
  });

  it('should return the default token limit if no model is provided', () => {
    // @ts-expect-error testing invalid input
    expect(tokenLimit(undefined)).toBe(DEFAULT_TOKEN_LIMIT);
  });

  it('should have the correct default token limit value', () => {
    expect(DEFAULT_TOKEN_LIMIT).toBe(1_048_576);
  });
});

describe('outputTokenLimit', () => {
  it('should return 65,536 for default M31A models and aliases', () => {
    expect(outputTokenLimit(DEFAULT_M31A_MODEL)).toBe(65_536);
    expect(outputTokenLimit(DEFAULT_M31A_MODEL)).toBe(65_536);
    expect(outputTokenLimit(DEFAULT_M31A_FLASH_MODEL)).toBe(65_536);
    expect(outputTokenLimit(DEFAULT_M31A_FLASH_LITE_MODEL)).toBe(65_536);
    expect(outputTokenLimit(M31A_MODEL_ALIAS_AUTO)).toBe(65_536);
    expect(outputTokenLimit(M31A_MODEL_ALIAS_PRO)).toBe(65_536);
    expect(outputTokenLimit(M31A_MODEL_ALIAS_FLASH)).toBe(65_536);
    expect(outputTokenLimit(M31A_MODEL_ALIAS_FLASH_LITE)).toBe(65_536);
  });

  it('should return undefined for unknown or custom models', () => {
    expect(outputTokenLimit('unknown-model')).toBeUndefined();
    expect(outputTokenLimit('custom-other-model')).toBeUndefined();
    // @ts-expect-error testing invalid input
    expect(outputTokenLimit(undefined)).toBeUndefined();
  });

  it('should return catalog output limits for known NVIDIA NIM models', () => {
    expect(outputTokenLimit('nvidia/nemotron-3.5-super-vl-120b-a12b')).toBe(
      8_192,
    );
    expect(outputTokenLimit('nvidia/nemotron-3.5-lightning-30b-a3b')).toBe(
      262_144,
    );
    expect(outputTokenLimit('deepseek-ai/deepseek-v4.1-flash')).toBe(384_000);
  });

  it('should have the correct default output token limit value', () => {
    expect(DEFAULT_OUTPUT_TOKEN_LIMIT).toBe(65_536);
  });
});
