/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, vi } from 'vitest';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { formatEditedFile, BUILTIN_FORMATTERS } from './formatterService.js';

describe('formatterService', () => {
  it('defines formatters for standard web, systems, and script extensions', () => {
    const names = BUILTIN_FORMATTERS.map((f) => f.name);
    expect(names).toContain('biome');
    expect(names).toContain('prettier');
    expect(names).toContain('gofmt');
    expect(names).toContain('rustfmt');
    expect(names).toContain('ruff');
  });

  it('invokes a local node_modules/.bin formatter matching the edited file extension', async () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fmt-service-test-'));
    try {
      const binDir = path.join(tmpDir, 'node_modules', '.bin');
      fs.mkdirSync(binDir, { recursive: true });
      const prettierBin = path.join(
        binDir,
        process.platform === 'win32' ? 'prettier.cmd' : 'prettier',
      );
      fs.writeFileSync(prettierBin, '');
      const targetFile = path.join(tmpDir, 'index.ts');
      fs.writeFileSync(targetFile, 'const x=1\n');

      const mockExec = vi.fn().mockResolvedValue({ exitCode: 0 });
      const used = await formatEditedFile(targetFile, tmpDir, {
        force: true,
        exec: mockExec,
      });

      expect(used).toBe('prettier');
      expect(mockExec).toHaveBeenCalledWith(
        prettierBin,
        ['--write', targetFile],
        tmpDir,
        5000,
      );
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  it('returns null gracefully when formatter execution fails or no extension exists', async () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fmt-service-fail-'));
    try {
      const binDir = path.join(tmpDir, 'node_modules', '.bin');
      fs.mkdirSync(binDir, { recursive: true });
      const prettierBin = path.join(
        binDir,
        process.platform === 'win32' ? 'prettier.cmd' : 'prettier',
      );
      fs.writeFileSync(prettierBin, '');
      const targetFile = path.join(tmpDir, 'app.ts');
      fs.writeFileSync(targetFile, 'export {}\n');

      const failingExec = vi.fn().mockRejectedValue(new Error('spawn failed'));
      const used = await formatEditedFile(targetFile, tmpDir, {
        force: true,
        exec: failingExec,
      });

      expect(used).toBeNull();
      expect(
        await formatEditedFile(path.join(tmpDir, 'README'), tmpDir, {
          force: true,
          exec: failingExec,
        }),
      ).toBeNull();
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });
});
