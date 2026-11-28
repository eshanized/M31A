/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Storage } from '@m31a/core';
import { createMockCommandContext } from '../../test-utils/mockCommandContext.js';
import { stashCommand } from './stashCommand.js';

describe('stashCommand', () => {
  let tempDir: string;

  beforeEach(async () => {
    tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'm31a-stash-cmd-'));
    vi.spyOn(Storage, 'getGlobalM31aDir').mockReturnValue(tempDir);
  });

  afterEach(async () => {
    vi.restoreAllMocks();
    await fs.rm(tempDir, { recursive: true, force: true });
  });

  it('saves, lists, pops into input, and clears prompt stash entries', async () => {
    const setInputText = vi.fn();
    const context = createMockCommandContext();
    context.ui.setInputText = setInputText;

    const emptyList = await stashCommand.action?.(context, 'list');
    expect(emptyList).toEqual({
      type: 'message',
      messageType: 'info',
      content: 'Prompt stash is empty. Use `/stash save <prompt>` to save one.',
    });

    const saved = await stashCommand.action?.(
      context,
      'save refactor parser safely',
    );
    expect(saved).toMatchObject({
      type: 'message',
      messageType: 'info',
    });

    const listed = await stashCommand.action?.(context, 'list');
    expect(listed).toMatchObject({
      type: 'message',
      messageType: 'info',
    });
    if (listed && listed.type === 'message') {
      expect(listed.content).toContain('refactor parser safely');
    }

    const popped = await stashCommand.action?.(context, 'pop');
    expect(popped).toMatchObject({
      type: 'message',
      messageType: 'info',
    });
    expect(setInputText).toHaveBeenCalledWith('refactor parser safely');

    const cleared = await stashCommand.action?.(context, 'clear');
    expect(cleared).toMatchObject({
      type: 'message',
      messageType: 'info',
      content: 'Cleared all stashed prompts.',
    });
  });
});
