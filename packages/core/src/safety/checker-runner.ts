/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type { FunctionCall } from '@m31a/genai';
import type {
  SafetyCheckerConfig,
  InProcessCheckerConfig,
} from '../policy/types.js';
import {
  SafetyCheckDecision,
  type SafetyCheckInput,
  type SafetyCheckResult,
} from './protocol.js';
import type { CheckerRegistry } from './registry.js';
import type { ContextBuilder } from './context-builder.js';

/**
 * Configuration for the checker runner.
 */
export interface CheckerRunnerConfig {
  /**
   * Maximum time (in milliseconds) to wait for a checker to complete.
   * Default: 5000 (5 seconds)
   */
  timeout?: number;
}

/**
 * Service for executing safety checkers.
 */
export class CheckerRunner {
  private static readonly DEFAULT_TIMEOUT = 5000; // 5 seconds

  private readonly registry: CheckerRegistry;
  private readonly contextBuilder: ContextBuilder;
  private readonly timeout: number;

  constructor(
    contextBuilder: ContextBuilder,
    registry: CheckerRegistry,
    config: CheckerRunnerConfig = {},
  ) {
    this.contextBuilder = contextBuilder;
    this.registry = registry;
    this.timeout = config.timeout ?? CheckerRunner.DEFAULT_TIMEOUT;
  }

  /**
   * Runs a safety checker and returns the result.
   */
  async runChecker(
    toolCall: FunctionCall,
    checkerConfig: SafetyCheckerConfig,
  ): Promise<SafetyCheckResult> {
    if (checkerConfig.type === 'in-process') {
      return this.runInProcessChecker(toolCall, checkerConfig);
    }
    return {
      decision: SafetyCheckDecision.DENY,
      reason: `External safety checkers are not supported ("${checkerConfig.name}").`,
    };
  }

  private async runInProcessChecker(
    toolCall: FunctionCall,
    checkerConfig: InProcessCheckerConfig,
  ): Promise<SafetyCheckResult> {
    try {
      const checker = this.registry.resolveInProcess(checkerConfig.name);
      const context = checkerConfig.required_context
        ? this.contextBuilder.buildMinimalContext(
            checkerConfig.required_context,
          )
        : this.contextBuilder.buildFullContext();

      const input: SafetyCheckInput = {
        protocolVersion: '1.0.0',
        toolCall,
        context,
        config: checkerConfig.config,
      };

      // In-process checkers can be async, but we'll also apply a timeout
      // for safety, in case of infinite loops or unexpected delays.
      return await this.executeWithTimeout(checker.check(input));
    } catch (error) {
      return {
        decision: SafetyCheckDecision.DENY,
        reason: `Failed to run in-process checker "${checkerConfig.name}": ${
          error instanceof Error ? error.message : String(error)
        }`,
      };
    }
  }

  /**
   * Executes a promise with a timeout.
   */
  private executeWithTimeout<T>(promise: Promise<T>): Promise<T> {
    return new Promise((resolve, reject) => {
      const timeoutHandle = setTimeout(() => {
        reject(new Error(`Checker timed out after ${this.timeout}ms`));
      }, this.timeout);

      promise
        .then(resolve)
        .catch(reject)
        .finally(() => {
          clearTimeout(timeoutHandle);
        });
    });
  }
}
