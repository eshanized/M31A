/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, expect, it, vi } from 'vitest';
import { createMockCommandContext } from '../../test-utils/mockCommandContext.js';
import { contextWindowCommand } from './contextWindowCommand.js';

describe('contextWindowCommand', () => {
  it('parses shorthand token values, resets on auto, and rejects small/invalid sizes', async () => {
    const setContextWindowOverride = vi.fn();
    const setValue = vi.fn();
    const context = createMockCommandContext({
      services: {
        agentContext: {
          config: {
            getModel: () => 'nvidia/nemotron-3-ultra-550b-a55b',
            getContextWindowOverride: () => undefined,
            setContextWindowOverride,
          },
        } as never,
        settings: {
          merged: { model: {} },
          setValue,
        } as never,
      },
    });

    const set128k = await contextWindowCommand.action?.(context, '128k');
    expect(set128k).toMatchObject({ type: 'message', messageType: 'info' });
    expect(setContextWindowOverride).toHaveBeenCalledWith(131_072, true);

    const resetAuto = await contextWindowCommand.action?.(context, 'auto');
    expect(resetAuto).toMatchObject({ type: 'message', messageType: 'info' });
    expect(setContextWindowOverride).toHaveBeenCalledWith(undefined, true);

    const tooSmall = await contextWindowCommand.action?.(context, '512');
    expect(tooSmall).toMatchObject({ type: 'message', messageType: 'error' });
  });
});
