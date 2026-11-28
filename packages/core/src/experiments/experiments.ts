/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type { ListExperimentsResponse, Flag } from './types.js';
import * as fs from 'node:fs';
import { debugLogger } from '../utils/debugLogger.js';

export interface Experiments {
  flags: Record<string, Flag>;
  experimentIds: number[];
}

let experimentsPromise: Promise<Experiments> | undefined;

/**
 * Gets local experiments from M31A_EXP if configured, otherwise returns empty flags.
 * Cached so it is only read once.
 */
export async function getExperiments(): Promise<Experiments> {
  if (experimentsPromise) {
    return experimentsPromise;
  }

  experimentsPromise = (async () => {
    if (process.env['M31A_EXP']) {
      try {
        const expPath = process.env['M31A_EXP'];
        debugLogger.debug('Reading experiments from', expPath);
        const content = await fs.promises.readFile(expPath, 'utf8');
        const parsed: unknown = JSON.parse(content);
        if (typeof parsed !== 'object' || parsed === null) {
          throw new Error('Invalid format for experiments file');
        }
        const response = parsed as ListExperimentsResponse;
        if (
          (response.flags && !Array.isArray(response.flags)) ||
          (response.experimentIds && !Array.isArray(response.experimentIds))
        ) {
          throw new Error(
            'Invalid format for experiments file: `flags` and `experimentIds` must be arrays if present.',
          );
        }
        return parseExperiments(response);
      } catch (e) {
        debugLogger.debug('Failed to read experiments from M31A_EXP', e);
      }
    }

    return { flags: {}, experimentIds: [] };
  })();
  return experimentsPromise;
}

function parseExperiments(response: ListExperimentsResponse): Experiments {
  const flags: Record<string, Flag> = {};
  for (const flag of response.flags ?? []) {
    if (flag.flagId) {
      flags[flag.flagId] = flag;
    }
  }
  return {
    flags,
    experimentIds: response.experimentIds ?? [],
  };
}
