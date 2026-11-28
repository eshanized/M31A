/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { GetInternalDocsTool } from './get-internal-docs.js';
import { ToolErrorType } from './tool-error.js';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { createMockMessageBus } from '../test-utils/mock-message-bus.js';

describe('GetInternalDocsTool (Integration)', () => {
  let tempDocsDir: string;
  let tool: GetInternalDocsTool;
  const abortSignal = new AbortController().signal;

  beforeEach(async () => {
    tempDocsDir = await fs.mkdtemp(path.join(os.tmpdir(), 'm31a-docs-test-'));
    await fs.writeFile(path.join(tempDocsDir, 'sidebar.json'), '{}', 'utf8');
    await fs.writeFile(
      path.join(tempDocsDir, 'index.md'),
      '# M31A Internal Documentation\n',
      'utf8',
    );
    await fs.mkdir(path.join(tempDocsDir, 'cli'), { recursive: true });
    await fs.writeFile(
      path.join(tempDocsDir, 'cli', 'commands.md'),
      '# Commands\n',
      'utf8',
    );
    tool = new GetInternalDocsTool(createMockMessageBus(), tempDocsDir);
  });

  afterEach(async () => {
    await fs.rm(tempDocsDir, { recursive: true, force: true });
  });

  it('should find the documentation root and list files', async () => {
    const invocation = tool.build({});
    const result = await invocation.execute({ abortSignal });

    expect(result.error).toBeUndefined();
    expect(result.returnDisplay).toMatch(/Found \d+ documentation files/);

    const content = result.llmContent as string;
    expect(content).toContain('index.md');
    expect(content).toContain('cli/commands.md');
  });

  it('should read a specific documentation file', async () => {
    const expectedContent = await fs.readFile(
      path.join(tempDocsDir, 'index.md'),
      'utf8',
    );

    const invocation = tool.build({ path: 'index.md' });
    const result = await invocation.execute({ abortSignal });

    expect(result.error).toBeUndefined();
    expect(result.llmContent).toBe(expectedContent);
    expect(result.returnDisplay).toContain('index.md');
  });

  it('should prevent access to files outside the docs directory (Path Traversal)', async () => {
    const invocation = tool.build({ path: '../package.json' });
    const result = await invocation.execute({ abortSignal });

    expect(result.error).toBeDefined();
    expect(result.error?.type).toBe(ToolErrorType.EXECUTION_FAILED);
    expect(result.error?.message).toContain('Access denied');
  });

  it('should handle non-existent files', async () => {
    const invocation = tool.build({ path: 'this-file-does-not-exist.md' });
    const result = await invocation.execute({ abortSignal });

    expect(result.error).toBeDefined();
    expect(result.error?.type).toBe(ToolErrorType.EXECUTION_FAILED);
  });

  it('returns an explicit error when no documentation directory is present', async () => {
    const defaultTool = new GetInternalDocsTool(createMockMessageBus());
    const invocation = defaultTool.build({});
    const result = await invocation.execute({ abortSignal });

    expect(result.error).toBeDefined();
    expect(result.error?.type).toBe(ToolErrorType.EXECUTION_FAILED);
    expect(result.error?.message).toContain('documentation directory');
  });
});
