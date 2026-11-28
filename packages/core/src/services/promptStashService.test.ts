/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { z } from 'zod';
import {
  PromptStashService,
  appendPromptHistoryEntry,
  loadPromptDraft,
  readPromptHistoryEntries,
  readSafeJsonlLines,
  savePromptDraft,
} from './promptStashService.js';

describe('promptStashService', () => {
  let tempDir: string;

  beforeEach(async () => {
    tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'm31a-prompt-stash-'));
  });

  afterEach(async () => {
    await fs.rm(tempDir, { recursive: true, force: true });
  });

  it('skips corrupt and truncated JSONL lines safely', () => {
    const raw = [
      '{"id":"1","input":"hello","timestamp":10}',
      '{truncated-json',
      '   ',
      '{"id":123,"input":"bad"}',
      '{"id":"2","input":"world","timestamp":20}',
    ].join('\n');

    const EntrySchema = z.object({ id: z.string(), input: z.string() });
    const parsed = readSafeJsonlLines(raw, (v) => {
      const res = EntrySchema.safeParse(v);
      return res.success ? res.data : undefined;
    });

    expect(parsed).toHaveLength(2);
    expect(parsed.map((item) => item.input)).toEqual(['hello', 'world']);
  });

  it('pushes, lists, pops, removes, and clears stashed prompts', async () => {
    const service = new PromptStashService(tempDir, { maxStashEntries: 3 });
    expect(await service.list()).toEqual([]);

    const first = await service.push('first prompt');
    await service.push('second prompt');
    await service.push('third prompt');
    await service.push('fourth prompt');

    const list = await service.list();
    expect(list).toHaveLength(3);
    expect(list.map((e) => e.input)).toEqual([
      'second prompt',
      'third prompt',
      'fourth prompt',
    ]);
    expect(await service.remove(first.id)).toBe(false);

    const popped = await service.pop();
    expect(popped?.input).toBe('fourth prompt');
    expect((await service.list()).map((e) => e.input)).toEqual([
      'second prompt',
      'third prompt',
    ]);

    expect(await service.remove(0)).toBe(true);
    expect((await service.list()).map((e) => e.input)).toEqual([
      'third prompt',
    ]);

    await service.clear();
    expect(await service.list()).toEqual([]);
  });

  it('heals corrupted prompt-history.jsonl and deduplicates consecutive entries', async () => {
    const service = new PromptStashService(tempDir);
    await fs.writeFile(
      service.historyFilePath,
      '{"input":"old cmd","timestamp":1}\n{corrupt line\n',
      'utf-8',
    );

    await appendPromptHistoryEntry(tempDir, 'new cmd');
    await appendPromptHistoryEntry(tempDir, 'new cmd');
    const entries = await readPromptHistoryEntries(tempDir);
    expect(entries.map((e) => e.input)).toEqual(['old cmd', 'new cmd']);
  });

  it('saves and loads prompt drafts', async () => {
    expect(await loadPromptDraft(tempDir)).toBe('');
    await savePromptDraft(tempDir, 'unfinished prompt');
    expect(await loadPromptDraft(tempDir)).toBe('unfinished prompt');
  });
});
