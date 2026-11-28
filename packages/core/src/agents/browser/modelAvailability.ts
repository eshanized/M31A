/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

/**
 * @fileoverview Model configuration for browser agent.
 *
 * Provides the default visual agent model and utilities for resolving
 * the configured model.
 */

import type { Config } from '../../config/config.js';
import { DEFAULT_M31A_MODEL } from '../../config/models.js';
import { debugLogger } from '../../utils/debugLogger.js';

/**
 * Default model for the visual agent.
 */
export const VISUAL_AGENT_MODEL = DEFAULT_M31A_MODEL;

/**
 * Pattern matching legacy computer-use model identifiers.
 */
const COMPUTER_USE_MODEL_PATTERN = /^nvidia\/.*-computer-use-/;

/**
 * Returns true if the model name belongs to the computer-use family
 * (matches gemini-*-computer-use-*).
 */
export function isComputerUseModel(model: string): boolean {
  return COMPUTER_USE_MODEL_PATTERN.test(model);
}

/**
 * Gets the visual agent model from config, falling back to default.
 *
 * @param config Runtime configuration
 * @returns The model to use for visual agent
 */
export function getVisualAgentModel(config: Config): string {
  const browserConfig = config.getBrowserAgentConfig();
  const model = browserConfig.customConfig.visualModel ?? VISUAL_AGENT_MODEL;

  debugLogger.log(`Visual agent model: ${model}`);
  return model;
}
