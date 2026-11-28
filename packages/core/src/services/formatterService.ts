/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { debugLogger } from '../utils/debugLogger.js';

export interface FormatterDefinition {
  readonly name: string;
  readonly extensions: readonly string[];
  resolveCommand: (
    filePath: string,
    targetDir: string,
  ) => { command: string; args: string[] } | null;
}

export type FormatterExec = (
  command: string,
  args: readonly string[],
  cwd: string,
  timeoutMs: number,
) => Promise<{ exitCode: number | null }>;

export interface FormatOptions {
  /** Force running even inside Vitest or when M31A_AUTO_FORMAT is unset. */
  force?: boolean;
  /** Custom executor for testing. */
  exec?: FormatterExec;
  /** Custom timeout in milliseconds (default: 5000). */
  timeoutMs?: number;
}

const DEFAULT_FORMAT_TIMEOUT_MS = 5000;

function findLocalNodeBin(
  binName: string,
  startDir: string,
  stopDir: string,
): string | null {
  let current = path.resolve(startDir);
  const boundary = path.resolve(stopDir);
  const execSuffix = process.platform === 'win32' ? '.cmd' : '';

  while (true) {
    const candidate = path.join(
      current,
      'node_modules',
      '.bin',
      `${binName}${execSuffix}`,
    );
    if (fs.existsSync(candidate)) {
      return candidate;
    }
    if (current === boundary) {
      break;
    }
    const parent = path.dirname(current);
    if (parent === current) {
      break;
    }
    current = parent;
  }
  return null;
}

function findOnPath(binName: string): string | null {
  const envPath = process.env['PATH'] ?? '';
  const dirs = envPath.split(path.delimiter).filter(Boolean);
  const suffixes =
    process.platform === 'win32' ? ['.exe', '.cmd', '.bat', ''] : [''];

  for (const dir of dirs) {
    for (const suffix of suffixes) {
      const candidate = path.join(dir, `${binName}${suffix}`);
      try {
        fs.accessSync(candidate, fs.constants.X_OK);
        return candidate;
      } catch {
        // Not executable at this location
      }
    }
  }
  return null;
}

export const BUILTIN_FORMATTERS: readonly FormatterDefinition[] = [
  {
    name: 'biome',
    extensions: [
      '.js',
      '.jsx',
      '.mjs',
      '.cjs',
      '.ts',
      '.tsx',
      '.mts',
      '.cts',
      '.json',
      '.jsonc',
      '.css',
    ],
    resolveCommand: (filePath, targetDir) => {
      const bin = findLocalNodeBin('biome', path.dirname(filePath), targetDir);
      return bin
        ? { command: bin, args: ['format', '--write', filePath] }
        : null;
    },
  },
  {
    name: 'prettier',
    extensions: [
      '.js',
      '.jsx',
      '.mjs',
      '.cjs',
      '.ts',
      '.tsx',
      '.mts',
      '.cts',
      '.json',
      '.jsonc',
      '.css',
      '.scss',
      '.html',
      '.yaml',
      '.yml',
      '.md',
      '.mdx',
    ],
    resolveCommand: (filePath, targetDir) => {
      const bin = findLocalNodeBin(
        'prettier',
        path.dirname(filePath),
        targetDir,
      );
      return bin ? { command: bin, args: ['--write', filePath] } : null;
    },
  },
  {
    name: 'gofmt',
    extensions: ['.go'],
    resolveCommand: (filePath) => {
      const bin = findOnPath('gofmt');
      return bin ? { command: bin, args: ['-w', filePath] } : null;
    },
  },
  {
    name: 'rustfmt',
    extensions: ['.rs'],
    resolveCommand: (filePath) => {
      const bin = findOnPath('rustfmt');
      return bin ? { command: bin, args: [filePath] } : null;
    },
  },
  {
    name: 'ruff',
    extensions: ['.py'],
    resolveCommand: (filePath) => {
      const bin = findOnPath('ruff');
      return bin ? { command: bin, args: ['format', filePath] } : null;
    },
  },
];

function defaultExec(
  command: string,
  args: readonly string[],
  cwd: string,
  timeoutMs: number,
): Promise<{ exitCode: number | null }> {
  return new Promise((resolve) => {
    let settled = false;
    const child = spawn(command, args, {
      cwd,
      stdio: 'ignore',
      windowsHide: true,
    });

    const timer = setTimeout(() => {
      if (!settled) {
        settled = true;
        child.kill('SIGTERM');
        resolve({ exitCode: null });
      }
    }, timeoutMs);

    child.on('error', () => {
      clearTimeout(timer);
      if (!settled) {
        settled = true;
        resolve({ exitCode: null });
      }
    });

    child.on('close', (code) => {
      clearTimeout(timer);
      if (!settled) {
        settled = true;
        resolve({ exitCode: code });
      }
    });
  });
}

/**
 * Formats an edited file in place using an installed project formatter matching its extension.
 * Safe and best-effort: never throws or blocks edits if no formatter exists or formatting fails.
 */
export async function formatEditedFile(
  filePath: string,
  targetDir: string,
  options: FormatOptions = {},
): Promise<string | null> {
  if (!options.force) {
    if (process.env['M31A_AUTO_FORMAT'] === 'false') {
      return null;
    }
    if (process.env['VITEST'] && process.env['M31A_AUTO_FORMAT'] !== 'true') {
      return null;
    }
  }

  const ext = path.extname(filePath).toLowerCase();
  if (!ext) {
    return null;
  }

  const exec = options.exec ?? defaultExec;
  const timeoutMs = options.timeoutMs ?? DEFAULT_FORMAT_TIMEOUT_MS;

  for (const formatter of BUILTIN_FORMATTERS) {
    if (!formatter.extensions.includes(ext)) {
      continue;
    }
    const resolved = formatter.resolveCommand(filePath, targetDir);
    if (!resolved) {
      continue;
    }

    try {
      const result = await exec(
        resolved.command,
        resolved.args,
        targetDir,
        timeoutMs,
      );
      if (result.exitCode === 0) {
        return formatter.name;
      }
    } catch (error) {
      debugLogger.warn(
        `Formatter ${formatter.name} failed for ${filePath}:`,
        error,
      );
    }
  }

  return null;
}
