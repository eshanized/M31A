/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect } from 'vitest';
import {
  createDefaultPolicy,
  getModelPolicyChain,
  getFlashLitePolicyChain,
  validateModelPolicyChain,
} from './policyCatalog.js';
import { DEFAULT_M31A_MODEL } from '../config/models.js';

describe('policyCatalog', () => {
  it('returns default NIM chain when preview enabled or disabled', () => {
    const previewChain = getModelPolicyChain({ previewEnabled: true });
    expect(previewChain[0]?.model).toBe(DEFAULT_M31A_MODEL);
    expect(previewChain).toHaveLength(1);
    expect(previewChain[0]?.isLastResort).toBe(true);

    const defaultChain = getModelPolicyChain({ previewEnabled: false });
    expect(defaultChain[0]?.model).toBe(DEFAULT_M31A_MODEL);
    expect(defaultChain).toHaveLength(1);
  });

  it('marks transients as sticky retries when auto-selected', () => {
    const [policy] = getModelPolicyChain({
      isAutoSelection: true,
    });
    expect(policy.model).toBe(DEFAULT_M31A_MODEL);
    expect(policy.stateTransitions.transient).toBe('sticky_retry');
  });

  it('applies default actions and state transitions for unspecified kinds', () => {
    const [policy] = getModelPolicyChain({});
    expect(policy.stateTransitions.not_found).toBe('terminal');
    expect(policy.stateTransitions.unknown).toBe('terminal');
    expect(policy.actions.unknown).toBe('prompt');
  });

  it('clones policy maps so edits do not leak between calls', () => {
    const firstCall = getModelPolicyChain({ previewEnabled: false });
    firstCall[0].actions.terminal = 'silent';
    const secondCall = getModelPolicyChain({ previewEnabled: false });
    expect(secondCall[0].actions.terminal).toBe('prompt');
  });

  it('returns DEFAULT_M31A_MODEL for getFlashLitePolicyChain', () => {
    const chain = getFlashLitePolicyChain();
    expect(chain[0]?.model).toBe(DEFAULT_M31A_MODEL);
  });

  it('passes when there is exactly one last-resort policy', () => {
    const validChain = [
      createDefaultPolicy('test-model'),
      { ...createDefaultPolicy('last-resort'), isLastResort: true },
    ];
    expect(() => validateModelPolicyChain(validChain)).not.toThrow();
  });

  it('fails when no policies are marked last-resort', () => {
    const chain = [
      createDefaultPolicy('model-a'),
      createDefaultPolicy('model-b'),
    ];
    expect(() => validateModelPolicyChain(chain)).toThrow(
      'must include an `isLastResort`',
    );
  });

  it('fails when a single-model chain is not last-resort', () => {
    const chain = [createDefaultPolicy('lonely-model')];
    expect(() => validateModelPolicyChain(chain)).toThrow(
      'must include an `isLastResort`',
    );
  });

  it('fails when multiple policies are marked last-resort', () => {
    const chain = [
      { ...createDefaultPolicy('model-a'), isLastResort: true },
      { ...createDefaultPolicy('model-b'), isLastResort: true },
    ];
    expect(() => validateModelPolicyChain(chain)).toThrow(
      'must only have one `isLastResort`',
    );
  });

  it('createDefaultPolicy seeds default actions and states', () => {
    const policy = createDefaultPolicy('custom');
    expect(policy.actions.terminal).toBe('prompt');
    expect(policy.actions.unknown).toBe('prompt');
    expect(policy.stateTransitions.terminal).toBe('terminal');
    expect(policy.stateTransitions.unknown).toBe('terminal');
  });
});
