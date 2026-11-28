/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Config } from '../config/config.js';
import { Storage } from '../config/storage.js';
import { createMockMessageBus } from '../test-utils/mock-message-bus.js';
import { SessionSearchTool } from './session-search.js';

describe('SessionSearchTool', () => {
  let tempDir: string;
  let globalDir: string;
  let projectTempDir: string;
  let mockConfig: Config;

  beforeEach(async () => {
    tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'm31a-sess-tool-'));
    globalDir = path.join(tempDir, 'global');
    projectTempDir = path.join(tempDir, 'project');
    await fs.mkdir(path.join(projectTempDir, 'chats'), { recursive: true });
    vi.spyOn(Storage, 'getGlobalM31aDir').mockReturnValue(globalDir);

    await fs.writeFile(
      path.join(projectTempDir, 'chats', 'session-101.json'),
      JSON.stringify({
        sessionId: 'session-101',
        summary: 'Implemented SQLite FTS5 session index',
        lastUpdated: '2026-11-28T04:00:00Z',
        messages: [
          {
            type: 'user',
            content: 'Use built-in node:sqlite DatabaseSync with WAL mode.',
          },
        ],
      }),
      'utf-8',
    );

    mockConfig = {
      getTargetDir: () => tempDir,
      storage: {
        getProjectTempDir: () => projectTempDir,
      },
    } as unknown as Config;
  });

  afterEach(async () => {
    vi.restoreAllMocks();
    await fs.rm(tempDir, { recursive: true, force: true });
  });

  it('indexes chats directory and returns matching FTS5 session hits', async () => {
    const tool = new SessionSearchTool(mockConfig, createMockMessageBus());
    const result = await tool
      .build({ query: 'DatabaseSync WAL', limit: 5 })
      .execute({ abortSignal: new AbortController().signal });

    expect(result.error).toBeUndefined();
    expect(String(result.llmContent)).toContain('session-101');
    expect(String(result.returnDisplay)).toContain('1 matching session');
  });

  it('rejects empty queries and out-of-range limits', () => {
    const tool = new SessionSearchTool(mockConfig, createMockMessageBus());
    expect(() => tool.build({ query: '   ' })).toThrow();
    expect(() => tool.build({ query: 'abc', limit: 0 })).toThrow();
  });
});
