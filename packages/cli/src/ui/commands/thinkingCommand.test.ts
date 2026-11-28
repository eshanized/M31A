/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, expect, it, vi } from 'vitest';
import { createMockCommandContext } from '../../test-utils/mockCommandContext.js';
import { thinkingCommand } from './thinkingCommand.js';

describe('thinkingCommand', () => {
  it('displays current thinking effort and updates valid levels cleanly', async () => {
    const setThinkingEffort = vi.fn();
    const setValue = vi.fn();
    const context = createMockCommandContext({
      services: {
        agentContext: {
          config: {
            getModel: () => 'nvidia/nemotron-3-ultra-550b-a55b',
            getThinkingEffort: () => 'default',
            setThinkingEffort,
          },
        } as never,
        settings: {
          merged: { model: { thinkingEffort: 'default' } },
          setValue,
        } as never,
      },
    });

    const info = await thinkingCommand.action?.(context, '');
    expect(info).toMatchObject({ type: 'message', messageType: 'info' });

    const setHigh = await thinkingCommand.action?.(context, 'high');
    expect(setHigh).toMatchObject({ type: 'message', messageType: 'info' });
    expect(setThinkingEffort).toHaveBeenCalledWith('high', true);

    const invalid = await thinkingCommand.action?.(context, 'ultra');
    expect(invalid).toMatchObject({ type: 'message', messageType: 'error' });
  });
});
