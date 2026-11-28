/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import * as fs from 'node:fs';

vi.mock('node:fs', async (importOriginal) => {
  const actual = await importOriginal<typeof import('node:fs')>();
  return {
    ...actual,
    promises: {
      ...actual.promises,
      readFile: vi.fn(),
    },
    readFileSync: vi.fn(),
  };
});

describe('experiments', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    delete process.env['M31A_EXP'];
  });

  afterEach(() => {
    delete process.env['M31A_EXP'];
  });

  it('should read experiments from local file if M31A_EXP is set', async () => {
    process.env['M31A_EXP'] = '/tmp/experiments.json';
    const mockFileContent = JSON.stringify({
      flags: [{ flagId: 111, boolValue: true }],
      experimentIds: [999],
    });
    vi.mocked(fs.promises.readFile).mockResolvedValue(mockFileContent);

    const { getExperiments } = await import('./experiments.js');
    const experiments = await getExperiments();

    expect(fs.promises.readFile).toHaveBeenCalledWith(
      '/tmp/experiments.json',
      'utf8',
    );
    expect(experiments.flags[111]).toEqual({
      flagId: 111,
      boolValue: true,
    });
    expect(experiments.experimentIds).toEqual([999]);
  });

  it('should return empty if no M31A_EXP is set', async () => {
    const { getExperiments } = await import('./experiments.js');
    const experiments = await getExperiments();
    expect(experiments.flags).toEqual({});
    expect(experiments.experimentIds).toEqual([]);
  });

  it('should return empty if reading file fails or has invalid structure', async () => {
    process.env['M31A_EXP'] = '/tmp/invalid.json';
    vi.mocked(fs.promises.readFile).mockResolvedValue(
      JSON.stringify({ flags: 'invalid', experimentIds: 123 }),
    );

    const { getExperiments } = await import('./experiments.js');
    const experiments = await getExperiments();
    expect(experiments.flags).toEqual({});
    expect(experiments.experimentIds).toEqual([]);
  });
});
