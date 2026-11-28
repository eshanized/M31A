/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { type AgentLoopContext } from '../config/agent-loop-context.js';
import { LocalAgentExecutor } from './local-executor.js';
import {
  BaseToolInvocation,
  type ToolResult,
  type ExecuteOptions,
} from '../tools/tools.js';
import {
  type LocalAgentDefinition,
  type AgentInputs,
  AgentTerminateMode,
  SubagentState,
} from './types.js';
import type { z } from 'zod';
import type { MessageBus } from '../confirmation-bus/message-bus.js';
import { SubagentActivityTracker } from './subagent-invocation-utils.js';
import { debugLogger } from '../utils/debugLogger.js';

/**
 * Represents a validated, executable instance of a subagent tool.
 *
 * This class orchestrates the execution of a defined agent by:
 * 1. Initializing the {@link LocalAgentExecutor}.
 * 2. Running the agent's execution loop.
 * 3. Bridging the agent's streaming activity (e.g., thoughts) to the tool's
 * live output stream.
 * 4. Formatting the final result into a {@link ToolResult}.
 */
export class LocalSubagentInvocation extends BaseToolInvocation<
  AgentInputs,
  ToolResult
> {
  /**
   * @param definition The definition object that configures the agent.
   * @param context The agent loop context.
   * @param params The validated input parameters for the agent.
   * @param messageBus Message bus for policy enforcement.
   */
  constructor(
    private readonly definition: LocalAgentDefinition,
    private readonly context: AgentLoopContext,
    params: AgentInputs,
    messageBus: MessageBus,
    _toolName?: string,
    _toolDisplayName?: string,
  ) {
    super(
      params,
      messageBus,
      _toolName ?? definition.name,
      _toolDisplayName ?? definition.displayName,
    );
  }

  /**
   * Returns a concise, human-readable description of the invocation.
   * Used for logging and display purposes.
   */
  getDescription(): string {
    const inputSummary = Object.entries(this.params)
      .map(([key, value]) => `${key}: ${String(value)}`)
      .join(', ');

    return `Running subagent '${this.definition.name}' with inputs: { ${inputSummary} }`;
  }

  async execute(options: ExecuteOptions): Promise<ToolResult> {
    const { abortSignal: signal, updateOutput } = options;
    const tracker = new SubagentActivityTracker({
      agentName: this.definition.name,
      displayName: this.definition.displayName,
      messageBus: this.messageBus,
      updateOutput,
    });
    let executor: LocalAgentExecutor<z.ZodUnknown> | undefined;

    try {
      if (updateOutput) {
        updateOutput(tracker.buildProgress(SubagentState.RUNNING));
      }

      executor = await LocalAgentExecutor.create(
        this.definition,
        this.context,
        (activity) => tracker.handleActivity(activity),
      );

      const output = await executor.run(this.params, signal);

      if (output.terminate_reason === AgentTerminateMode.ABORTED) {
        if (updateOutput) {
          updateOutput(tracker.buildProgress(SubagentState.CANCELLED));
        }

        const cancelError = new Error('Operation cancelled by user');
        cancelError.name = 'AbortError';
        throw cancelError;
      }

      const progress = tracker.buildProgress(SubagentState.COMPLETED, {
        result: output.result,
        terminateReason: output.terminate_reason,
      });

      if (updateOutput) {
        updateOutput(progress);
      }

      const resultContent = `Subagent '${this.definition.name}' finished.
Termination Reason: ${output.terminate_reason}
Result:
${output.result}`;

      return {
        llmContent: [{ text: resultContent }],
        returnDisplay: progress,
        data: { agentId: executor.agentId },
      };
    } catch (error) {
      debugLogger.error(`Subagent '${this.definition.name}' failed:`, error);
      const { isAbort, errorMessage, progress } = tracker.finalizeError(error);

      if (isAbort) {
        throw error;
      }

      return {
        llmContent: `Subagent '${this.definition.name}' failed. Error: ${errorMessage}`,
        returnDisplay: progress,
        data: executor ? { agentId: executor.agentId } : undefined,
        // We omit the 'error' property so that the UI renders our rich returnDisplay
        // instead of the raw error message. The llmContent still informs the agent of the failure.
      };
    }
  }
}
