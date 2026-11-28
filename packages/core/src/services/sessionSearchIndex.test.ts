/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { SessionSearchIndex } from './sessionSearchIndex.js';

describe('SessionSearchIndex (node:sqlite FTS5)', () => {
  let tempDir: string;

  beforeEach(async () => {
    tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'm31a-session-fts-'));
  });

  afterEach(async () => {
    await fs.rm(tempDir, { recursive: true, force: true });
  });

  it('indexes sessions into FTS5 and ranks matching snippets safely', async () => {
    const dbPath = path.join(tempDir, 'sessions-fts.sqlite');
    const index = await SessionSearchIndex.create(dbPath);

    try {
      index.upsertSession({
        sessionId: 'session-1',
        projectRoot: '/repo/a',
        summary: 'Refactor auth token rotation',
        content: 'Updated JWT refresh rotation and fixed middleware error.',
        updatedAt: '2026-11-28T01:00:00Z',
      });
      index.upsertSession({
        sessionId: 'session-2',
        projectRoot: '/repo/b',
        summary: 'Add mermaid diagram renderer',
        content: 'Rendered ASCII flowchart diagrams in terminal Markdown.',
        updatedAt: '2026-11-28T02:00:00Z',
      });

      const hits = index.search('mermaid flowchart');
      expect(hits).toHaveLength(1);
      expect(hits[0]?.sessionId).toBe('session-2');
      expect(hits[0]?.snippet).toContain('[flowchart]');

      // Punctuation should be sanitized cleanly without FTS syntax errors.
      expect(index.search('middleware: ("error")')).toHaveLength(1);

      const scoped = index.search('rotation', { projectRoot: '/repo/b' });
      expect(scoped).toHaveLength(0);

      index.removeSession('session-2');
      expect(index.search('mermaid')).toHaveLength(0);
    } finally {
      index.close();
    }
  });

  it('incrementally indexes session JSON files from a chats directory and skips unchanged files', async () => {
    const chatsDir = path.join(tempDir, 'chats');
    await fs.mkdir(chatsDir, { recursive: true });
    await fs.writeFile(
      path.join(chatsDir, 'session-abc.json'),
      JSON.stringify({
        sessionId: 'session-abc',
        summary: 'NVIDIA NIM streaming fix',
        lastUpdated: '2026-11-28T03:00:00Z',
        messages: [
          { type: 'user', content: 'Fix tool call delta buffering' },
          {
            type: 'gemini',
            content: [{ text: 'Updated adapter buffering for SSE streams.' }],
          },
        ],
      }),
      'utf-8',
    );
    await fs.writeFile(
      path.join(chatsDir, 'session-corrupt.json'),
      '{not-json',
      'utf-8',
    );

    const index = await SessionSearchIndex.create(':memory:');
    try {
      const firstCount = await index.indexDirectory(chatsDir, '/repo/m31a');
      expect(firstCount).toBe(1);

      const secondCount = await index.indexDirectory(chatsDir, '/repo/m31a');
      expect(secondCount).toBe(0);

      const hits = index.search('buffering');
      expect(hits).toHaveLength(1);
      expect(hits[0]?.sessionId).toBe('session-abc');
    } finally {
      index.close();
    }
  });
});
