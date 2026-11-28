/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import { z } from 'zod';
import { Storage } from '../config/storage.js';
import { isNodeError } from '../utils/errors.js';

export interface StashEntry {
  id: string;
  input: string;
  timestamp: number;
}

export interface PromptHistoryEntry {
  input: string;
  timestamp: number;
}

const StashEntrySchema = z.object({
  id: z.string(),
  input: z.string(),
  timestamp: z.number(),
});

const PromptHistoryEntrySchema = z.object({
  input: z.string(),
  timestamp: z.number(),
});

const DEFAULT_MAX_STASH_ENTRIES = 50;
const DEFAULT_MAX_HISTORY_ENTRIES = 500;

/**
 * Parses JSONL lines while skipping empty or corrupted/truncated lines so a
 * partial write never breaks history or stash recovery.
 */
export function readSafeJsonlLines<T>(
  raw: string,
  parser: (value: unknown) => T | undefined,
): T[] {
  const results: T[] = [];
  for (const line of raw.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed) {
      continue;
    }
    try {
      const parsed: unknown = JSON.parse(trimmed);
      const item = parser(parsed);
      if (item !== undefined) {
        results.push(item);
      }
    } catch {
      // Skip malformed/truncated lines gracefully.
    }
  }
  return results;
}

function parseStashEntry(value: unknown): StashEntry | undefined {
  const result = StashEntrySchema.safeParse(value);
  return result.success ? result.data : undefined;
}

function parsePromptHistoryEntry(
  value: unknown,
): PromptHistoryEntry | undefined {
  const result = PromptHistoryEntrySchema.safeParse(value);
  return result.success ? result.data : undefined;
}

async function readFileIfExists(filePath: string): Promise<string> {
  try {
    return await fs.readFile(filePath, 'utf-8');
  } catch (error) {
    if (isNodeError(error) && error.code === 'ENOENT') {
      return '';
    }
    throw error;
  }
}

async function writeAtomic(filePath: string, content: string): Promise<void> {
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  const tmpPath = `${filePath}.${process.pid}.${Date.now()}.tmp`;
  await fs.writeFile(tmpPath, content, 'utf-8');
  await fs.rename(tmpPath, filePath);
}

/**
 * Self-healing JSONL stash, prompt history, and draft persistence service.
 */
export class PromptStashService {
  readonly baseDir: string;
  readonly stashFilePath: string;
  readonly historyFilePath: string;
  readonly draftFilePath: string;
  readonly maxStashEntries: number;
  readonly maxHistoryEntries: number;

  constructor(
    baseDir: string = Storage.getGlobalM31aDir(),
    options?: { maxStashEntries?: number; maxHistoryEntries?: number },
  ) {
    this.baseDir = baseDir;
    this.stashFilePath = path.join(baseDir, 'prompt-stash.jsonl');
    this.historyFilePath = path.join(baseDir, 'prompt-history.jsonl');
    this.draftFilePath = path.join(baseDir, 'prompt-draft.txt');
    this.maxStashEntries =
      options?.maxStashEntries ?? DEFAULT_MAX_STASH_ENTRIES;
    this.maxHistoryEntries =
      options?.maxHistoryEntries ?? DEFAULT_MAX_HISTORY_ENTRIES;
  }

  async list(): Promise<StashEntry[]> {
    const raw = await readFileIfExists(this.stashFilePath);
    if (!raw) {
      return [];
    }
    return readSafeJsonlLines(raw, parseStashEntry);
  }

  async push(input: string): Promise<StashEntry> {
    const existing = await this.list();
    const entry: StashEntry = {
      id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      input,
      timestamp: Date.now(),
    };
    const next = [...existing, entry].slice(-this.maxStashEntries);
    const serialized =
      next.map((item) => JSON.stringify(item)).join('\n') +
      (next.length > 0 ? '\n' : '');
    await writeAtomic(this.stashFilePath, serialized);
    return entry;
  }

  async pop(): Promise<StashEntry | undefined> {
    const existing = await this.list();
    if (existing.length === 0) {
      return undefined;
    }
    const popped = existing[existing.length - 1];
    const remaining = existing.slice(0, -1);
    const serialized =
      remaining.map((item) => JSON.stringify(item)).join('\n') +
      (remaining.length > 0 ? '\n' : '');
    await writeAtomic(this.stashFilePath, serialized);
    return popped;
  }

  async remove(indexOrId: number | string): Promise<boolean> {
    const existing = await this.list();
    const index =
      typeof indexOrId === 'number'
        ? indexOrId
        : existing.findIndex((item) => item.id === indexOrId);
    if (index < 0 || index >= existing.length) {
      return false;
    }
    const next = existing.filter((_, idx) => idx !== index);
    const serialized =
      next.map((item) => JSON.stringify(item)).join('\n') +
      (next.length > 0 ? '\n' : '');
    await writeAtomic(this.stashFilePath, serialized);
    return true;
  }

  async clear(): Promise<void> {
    await writeAtomic(this.stashFilePath, '');
  }

  async appendHistory(input: string): Promise<void> {
    const trimmed = input.trim();
    if (!trimmed) {
      return;
    }
    const existing = await this.readHistory();
    if (
      existing.length > 0 &&
      existing[existing.length - 1]?.input === trimmed
    ) {
      return;
    }
    const entry: PromptHistoryEntry = {
      input: trimmed,
      timestamp: Date.now(),
    };
    const next = [...existing, entry].slice(-this.maxHistoryEntries);
    const serialized =
      next.map((item) => JSON.stringify(item)).join('\n') + '\n';
    await writeAtomic(this.historyFilePath, serialized);
  }

  async readHistory(): Promise<PromptHistoryEntry[]> {
    const raw = await readFileIfExists(this.historyFilePath);
    if (!raw) {
      return [];
    }
    return readSafeJsonlLines(raw, parsePromptHistoryEntry);
  }

  async saveDraft(text: string): Promise<void> {
    await writeAtomic(this.draftFilePath, text);
  }

  async loadDraft(): Promise<string> {
    return readFileIfExists(this.draftFilePath);
  }
}

export async function appendPromptHistoryEntry(
  baseDir: string,
  input: string,
): Promise<void> {
  const service = new PromptStashService(baseDir);
  await service.appendHistory(input);
}

export async function readPromptHistoryEntries(
  baseDir: string,
): Promise<PromptHistoryEntry[]> {
  const service = new PromptStashService(baseDir);
  return service.readHistory();
}

export async function savePromptDraft(
  baseDir: string,
  text: string,
): Promise<void> {
  const service = new PromptStashService(baseDir);
  await service.saveDraft(text);
}

export async function loadPromptDraft(baseDir: string): Promise<string> {
  const service = new PromptStashService(baseDir);
  return service.loadDraft();
}
