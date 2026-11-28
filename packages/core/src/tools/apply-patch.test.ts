/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  ApplyPatchTool,
  parsePatchText,
  deriveUpdatedContent,
} from './apply-patch.js';
import { createMockMessageBus } from '../test-utils/mock-message-bus.js';
import { createMockWorkspaceContext } from '../test-utils/mockWorkspaceContext.js';
import { StandardFileSystemService } from '../services/fileSystemService.js';
import { ApprovalMode } from '../policy/types.js';
import { resolveToRealPath } from '../utils/paths.js';
import type { Config } from '../config/config.js';
import { UTF8_BOM } from '../utils/bom.js';

describe('ApplyPatchTool', () => {
  let rootDir: string;
  let mockConfig: Config;
  let tool: ApplyPatchTool;

  beforeEach(() => {
    vi.restoreAllMocks();
    rootDir = resolveToRealPath(
      fs.mkdtempSync(path.join(os.tmpdir(), 'apply-patch-test-')),
    );
    const fsService = new StandardFileSystemService();
    mockConfig = {
      getTargetDir: () => rootDir,
      getProjectRoot: () => rootDir,
      getApprovalMode: () => ApprovalMode.YOLO,
      getFileSystemService: () => fsService,
      getWorkspaceContext: () => createMockWorkspaceContext(rootDir),
      getMemoryContextManager: () => null,
      validatePathAccess: () => null,
      isPlanMode: () => false,
    } as unknown as Config;

    tool = new ApplyPatchTool(mockConfig, createMockMessageBus());
  });

  afterEach(() => {
    fs.rmSync(rootDir, { recursive: true, force: true });
  });

  it('parses heredoc-wrapped patches and validates Add/Update/Move/Delete hunks', () => {
    const hunks = parsePatchText(`<<'EOF'
*** Begin Patch
*** Add File: created.txt
+hello
*** Update File: old.txt
*** Move to: moved.txt
@@
-a
+b
*** Delete File: dead.txt
*** End Patch
EOF`);
    expect(hunks).toHaveLength(3);
    expect(hunks[0]?.type).toBe('add');
    expect(hunks[1]).toMatchObject({
      type: 'update',
      path: 'old.txt',
      movePath: 'moved.txt',
    });
    expect(hunks[2]?.type).toBe('delete');
  });

  it('preserves UTF-8 BOM and matches smart typography in deriveUpdatedContent', () => {
    const original = `${UTF8_BOM}const msg = “hello”;\nconst keep = 1;\n`;
    const derived = deriveUpdatedContent(
      'sample.ts',
      [
        {
          oldLines: ['const msg = "hello";'],
          newLines: ['const msg = "world";'],
        },
      ],
      original,
    );
    expect(derived.bom).toBe(true);
    expect(derived.content).toBe('const msg = "world";\nconst keep = 1;\n');
  });

  it('executes add, update+move, and delete operations atomically across files', async () => {
    const updateSrc = path.join(rootDir, 'src.txt');
    const deleteFile = path.join(rootDir, 'remove.txt');
    fs.writeFileSync(updateSrc, `${UTF8_BOM}line 1\nline 2\n`);
    fs.writeFileSync(deleteFile, 'obsolete\n');

    const patch = `*** Begin Patch
*** Add File: sub/added.txt
+fresh line
*** Update File: src.txt
*** Move to: sub/renamed.txt
@@
 line 1
-line 2
+line two
*** Delete File: remove.txt
*** End Patch`;

    const invocation = tool.build({ patch_text: patch });
    const result = await invocation.execute({
      abortSignal: new AbortController().signal,
    });

    expect(result.error).toBeUndefined();
    expect(fs.readFileSync(path.join(rootDir, 'sub/added.txt'), 'utf8')).toBe(
      'fresh line\n',
    );
    expect(fs.readFileSync(path.join(rootDir, 'sub/renamed.txt'), 'utf8')).toBe(
      `${UTF8_BOM}line 1\nline two\n`,
    );
    expect(fs.existsSync(updateSrc)).toBe(false);
    expect(fs.existsSync(deleteFile)).toBe(false);
  });
});
