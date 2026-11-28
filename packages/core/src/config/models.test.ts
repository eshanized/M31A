/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect } from 'vitest';
import {
  resolveModel,
  resolveClassifierModel,
  isGemini3Model,
  isGemini2Model,
  isAutoModel,
  getDisplayString,
  DEFAULT_M31A_MODEL,
  DEFAULT_M31A_EMBEDDING_MODEL,
  DEFAULT_M31A_FLASH_MODEL,
  DEFAULT_M31A_FLASH_LITE_MODEL,
  supportsMultimodalFunctionResponse,
  M31A_MODEL_ALIAS_PRO,
  M31A_MODEL_ALIAS_FLASH,
  M31A_MODEL_ALIAS_FLASH_LITE,
  M31A_MODEL_ALIAS_AUTO,
  DEFAULT_M31A_MODEL_AUTO,
  isActiveModel,
  isPreviewModel,
  isProModel,
  VALID_M31A_MODELS,
  getAutoModelDescription,
} from './models.js';
import type { Config } from './config.js';
import { ModelConfigService } from '../services/modelConfigService.js';
import { DEFAULT_MODEL_CONFIGS } from './defaultModelConfigs.js';

const modelConfigService = new ModelConfigService(DEFAULT_MODEL_CONFIGS);

const dynamicConfig = {
  getExperimentalDynamicModelConfiguration: () => true,
  modelConfigService,
} as unknown as Config;

describe('M31A model defaults and resolution', () => {
  it('defaults to NVIDIA NIM nemotron-3-ultra-550b-a55b and nv-embed-v1', () => {
    expect(DEFAULT_M31A_MODEL).toBe('nvidia/nemotron-3-ultra-550b-a55b');
    expect(DEFAULT_M31A_EMBEDDING_MODEL).toBe('nvidia/nv-embed-v1');
    expect(DEFAULT_M31A_MODEL).toBe(DEFAULT_M31A_MODEL);
    expect(DEFAULT_M31A_FLASH_MODEL).toBe(DEFAULT_M31A_MODEL);
    expect(DEFAULT_M31A_FLASH_LITE_MODEL).toBe(DEFAULT_M31A_MODEL);
    expect(VALID_M31A_MODELS.has(DEFAULT_M31A_MODEL)).toBe(true);
    expect(VALID_M31A_MODELS.size).toBe(1);
  });

  it('resolves aliases and empty inputs to DEFAULT_M31A_MODEL', () => {
    expect(resolveModel(M31A_MODEL_ALIAS_AUTO)).toBe(DEFAULT_M31A_MODEL);
    expect(resolveModel(M31A_MODEL_ALIAS_PRO)).toBe(DEFAULT_M31A_MODEL);
    expect(resolveModel(M31A_MODEL_ALIAS_FLASH)).toBe(DEFAULT_M31A_MODEL);
    expect(resolveModel(M31A_MODEL_ALIAS_FLASH_LITE)).toBe(DEFAULT_M31A_MODEL);
    expect(resolveModel(DEFAULT_M31A_MODEL_AUTO)).toBe(DEFAULT_M31A_MODEL);
    expect(resolveModel('')).toBe(DEFAULT_M31A_MODEL);
    expect(resolveModel(['auto', 'nvidia/custom'] as unknown as string)).toBe(
      'nvidia/custom',
    );
  });

  it('preserves explicit custom and NVIDIA NIM model IDs', () => {
    expect(resolveModel('nvidia/llama-3.1-nemotron-70b-instruct')).toBe(
      'nvidia/llama-3.1-nemotron-70b-instruct',
    );
    expect(resolveModel('meta/llama-3.1-405b-instruct')).toBe(
      'meta/llama-3.1-405b-instruct',
    );
  });

  it('matches dynamic ModelConfigService resolution for aliases', () => {
    for (const model of [
      M31A_MODEL_ALIAS_AUTO,
      M31A_MODEL_ALIAS_PRO,
      M31A_MODEL_ALIAS_FLASH,
      M31A_MODEL_ALIAS_FLASH_LITE,
      DEFAULT_M31A_MODEL,
      'custom-model',
    ]) {
      expect(resolveModel(model, dynamicConfig)).toBe(resolveModel(model));
    }
  });

  it('resolves classifier models to DEFAULT_M31A_MODEL', () => {
    expect(
      resolveClassifierModel(M31A_MODEL_ALIAS_AUTO, M31A_MODEL_ALIAS_FLASH),
    ).toBe(DEFAULT_M31A_MODEL);
    expect(
      resolveClassifierModel(
        M31A_MODEL_ALIAS_AUTO,
        M31A_MODEL_ALIAS_PRO,
        dynamicConfig,
      ),
    ).toBe(DEFAULT_M31A_MODEL);
  });

  it('returns accurate display strings and auto descriptions', () => {
    expect(getDisplayString(M31A_MODEL_ALIAS_AUTO)).toBe('Auto');
    expect(getDisplayString(DEFAULT_M31A_MODEL)).toBe(DEFAULT_M31A_MODEL);
    expect(getDisplayString('nvidia/other')).toBe('nvidia/other');
    expect(getAutoModelDescription()).toBe(
      `Let M31A use the default model: ${DEFAULT_M31A_MODEL}`,
    );
  });

  it('reports model predicates cleanly for NIM defaults and custom models', () => {
    expect(isAutoModel(M31A_MODEL_ALIAS_AUTO)).toBe(true);
    expect(isAutoModel(DEFAULT_M31A_MODEL)).toBe(false);
    expect(isPreviewModel(DEFAULT_M31A_MODEL)).toBe(false);
    expect(isPreviewModel('none')).toBe(false);
    expect(isActiveModel(DEFAULT_M31A_MODEL)).toBe(true);
    expect(isActiveModel('unknown-model')).toBe(false);
    expect(isActiveModel('none')).toBe(false);
    expect(isProModel('my-pro-model')).toBe(true);
    expect(isGemini3Model('gemini-3-pro-preview')).toBe(true);
    expect(isGemini3Model(DEFAULT_M31A_MODEL)).toBe(false);
    expect(isGemini2Model('gemini-2.5-pro')).toBe(true);
    expect(isGemini2Model(DEFAULT_M31A_MODEL)).toBe(false);
    expect(supportsMultimodalFunctionResponse('gemini-3-pro-preview')).toBe(
      true,
    );
    expect(supportsMultimodalFunctionResponse(DEFAULT_M31A_MODEL)).toBe(false);
  });
});
