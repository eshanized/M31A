/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { type AgentLoopContext } from '../config/agent-loop-context.js';
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
import type { MessageBus } from '../confirmation-bus/message-bus.js';
import { LocalSubagentSession } from './local-subagent-protocol.js';
import { SubagentActivityTracker } from './subagent-invocation-utils.js';
import type { AgentEvent } from '../agent/types.js';

const MAX_RECENT_ACTIVITY = 3;

/** Optional configuration for subagent invocations. */
export interface SubagentInvocationOptions {
  toolName?: string;
  toolDisplayName?: string;
  onAgentEvent?: (event: AgentEvent) => void;
}

/**
 * Session-based local subagent invocation.
 *
 * This class orchestrates the execution of a defined agent by:
 * 1. Using {@link LocalSubagentSession} as the execution engine.
 * 2. Bridging the agent's streaming activity (e.g., thoughts) to the tool's
 *    live output stream via the session's rawActivityCallback.
 * 3. Formatting the final result into a {@link ToolResult}.
 */
export class LocalSessionInvocation extends BaseToolInvocation<
  AgentInputs,
  ToolResult
> {
  private readonly _onAgentEvent?: (event: AgentEvent) => void;

  /**
   * @param definition The definition object that configures the agent.
   * @param context The agent loop context.
   * @param params The validated input parameters for the agent.
   * @param messageBus Message bus for policy enforcement.
   * @param options Optional overrides for tool name, display name, and event callback.
   */
  constructor(
    private readonly definition: LocalAgentDefinition,
    private readonly context: AgentLoopContext,
    params: AgentInputs,
    messageBus: MessageBus,
    options?: SubagentInvocationOptions,
  ) {
    super(
      params,
      messageBus,
      options?.toolName ?? definition.name,
      options?.toolDisplayName ?? definition.displayName,
    );
    this._onAgentEvent = options?.onAgentEvent;
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
      maxRecentActivity: MAX_RECENT_ACTIVITY,
      trimToolNames: true,
    });

    // Create session with the raw activity callback for rich progress display
    const session = new LocalSubagentSession(
      this.definition,
      this.context,
      this.messageBus,
      (activity) => tracker.handleActivity(activity),
    );

    // Subscribe for parent session observability
    let unsubscribeParent: (() => void) | undefined;
    if (this._onAgentEvent) {
      unsubscribeParent = session.subscribe(this._onAgentEvent);
    }

    // Wire external abort signal to session abort
    const abortListener = () => void session.abort();
    signal.addEventListener('abort', abortListener, { once: true });

    try {
      if (updateOutput) {
        updateOutput(tracker.buildProgress(SubagentState.RUNNING));
      }

      // Buffer non-query params, then send query as message to start execution
      const query = String(this.params['query'] ?? '');
      const otherParams = { ...this.params } as Record<string, unknown>;
      delete otherParams['query'];
      if (Object.keys(otherParams).length > 0) {
        await session.send({ update: { config: otherParams } });
      }
      await session.send({
        message: { content: [{ type: 'text', text: query }] },
      });

      const output = await session.getResult();

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
      };
    } catch (error) {
      const { isAbort, errorMessage, progress } = tracker.finalizeError(error);

      if (isAbort) {
        throw error;
      }

      return {
        llmContent: `Subagent '${this.definition.name}' failed. Error: ${errorMessage}`,
        returnDisplay: progress,
      };
    } finally {
      signal.removeEventListener('abort', abortListener);
      unsubscribeParent?.();
    }
  }
}
