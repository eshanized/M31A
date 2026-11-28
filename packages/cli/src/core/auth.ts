/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import {
  type AuthType,
  type Config,
  getErrorMessage,
  ValidationRequiredError,
} from '@m31a/core';

export interface InitialAuthResult {
  authError: string | null;
}

/**
 * Handles the initial authentication flow.
 * @param config The application config.
 * @param authType The selected auth type.
 * @returns The auth result with error message.
 */
export async function performInitialAuth(
  config: Config,
  authType: AuthType | undefined,
): Promise<InitialAuthResult> {
  if (!authType) {
    return { authError: null };
  }

  try {
    await config.refreshAuth(authType);
  } catch (e) {
    if (e instanceof ValidationRequiredError) {
      return { authError: null };
    }
    return {
      authError: `Failed to sign in. Message: ${getErrorMessage(e)}`,
    };
  }

  return { authError: null };
}
