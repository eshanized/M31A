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
import { createMockMessageBus } from '../test-utils/mock-message-bus.js';
import { ExecuteTool, runSandboxedInspection } from './codemode-execute.js';

describe('ExecuteTool (codemode VM sandbox)', () => {
  let tempDir: string;
  let mockConfig: Config;

  beforeEach(async () => {
    tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'm31a-codemode-'));
    await fs.mkdir(path.join(tempDir, 'src'), { recursive: true });
    await fs.writeFile(
      path.join(tempDir, 'src', 'a.ts'),
      'export const alpha = 1;\nexport const beta = 2;\n',
      'utf-8',
    );
    await fs.writeFile(
      path.join(tempDir, 'src', 'b.ts'),
      'export const gamma = 3;\n',
      'utf-8',
    );

    mockConfig = {
      getTargetDir: () => tempDir,
      validatePathAccess: vi.fn((absPath: string) => {
        if (!absPath.startsWith(tempDir)) {
          return `Access denied outside workspace: ${absPath}`;
        }
        return null;
      }),
    } as unknown as Config;
  });

  afterEach(async () => {
    await fs.rm(tempDir, { recursive: true, force: true });
  });

  it('batches readFile, listDir, glob, and grep inside isolated VM and returns JSON', async () => {
    const tool = new ExecuteTool(mockConfig, createMockMessageBus());
    const invocation = tool.build({
      code: `
        const entries = await listDir('src');
        const files = await glob('src/**/*.ts');
        const firstLine = await readFile('src/a.ts', { startLine: 1, endLine: 1 });
        const matches = await grep('gamma', 'src');
        console.log('checked', files.length);
        return { entries, files, firstLine, matches };
      `,
    });

    const result = await invocation.execute({
      abortSignal: new AbortController().signal,
    });
    expect(result.error).toBeUndefined();
    expect(String(result.llmContent)).toContain('checked 2');
    expect(String(result.llmContent)).toContain('export const alpha = 1;');
    expect(String(result.llmContent)).toContain('b.ts');
  });

  it('blocks access outside workspace and isolates Node globals', async () => {
    await expect(
      runSandboxedInspection('return typeof process;', mockConfig),
    ).resolves.toMatchObject({ result: 'undefined' });

    const tool = new ExecuteTool(mockConfig, createMockMessageBus());
    const escapeResult = await tool
      .build({
        code: `return await readFile('/etc/passwd');`,
      })
      .execute({ abortSignal: new AbortController().signal });
    expect(escapeResult.error).toBeDefined();
    expect(String(escapeResult.llmContent)).toContain('Access denied outside');
  });

  it('enforces execution timeout on tight infinite loops', async () => {
    await expect(
      runSandboxedInspection('while (true) {}', mockConfig, 100),
    ).rejects.toThrow(/timed out/i);
  });
});
