/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { AuthType, loadApiKey } from '@m31a/core';
import { loadEnvironment, loadSettings } from './settings.js';

export async function validateAuthMethod(
  authMethod: string,
): Promise<string | null> {
  loadEnvironment(loadSettings().merged, process.cwd());

  if (authMethod === AuthType.USE_NVIDIA_NIM) {
    const key = process.env['NVIDIA_API_KEY'] || (await loadApiKey());
    if (!key) {
      return (
        'When using NVIDIA NIM, you must specify the NVIDIA_API_KEY environment variable.\n' +
        'Update your environment and try again (no reload needed if using .env)!'
      );
    }
    return null;
  }

  return 'Invalid auth method selected. M31A uses NVIDIA NIM only (NVIDIA_API_KEY).';
}
