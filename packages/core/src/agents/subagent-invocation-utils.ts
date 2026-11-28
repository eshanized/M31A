/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { randomUUID } from 'node:crypto';
import { MessageBusType } from '../confirmation-bus/types.js';
import type { MessageBus } from '../confirmation-bus/message-bus.js';
import { A2AAgentError } from './a2a-errors.js';
import {
  SUBAGENT_CANCELLED_ERROR_MESSAGE,
  SUBAGENT_REJECTED_ERROR_PREFIX,
  SubagentActivityErrorType,
  SubagentState,
  isToolActivityError,
  type SubagentActivityEvent,
  type SubagentActivityItem,
  type SubagentProgress,
} from './types.js';
import {
  sanitizeErrorMessage,
  sanitizeThoughtContent,
  sanitizeToolArgs,
} from '../utils/agent-sanitization-utils.js';

export interface SubagentActivityTrackerOptions {
  agentName: string;
  displayName?: string;
  messageBus: MessageBus;
  updateOutput?: (output: SubagentProgress) => void;
  maxRecentActivity?: number;
  trimToolNames?: boolean;
}

/**
 * Tracks streaming subagent activity items and bridges updates to the
 * confirmation message bus and tool progress callback.
 */
export class SubagentActivityTracker {
  recentActivity: SubagentActivityItem[] = [];

  constructor(private readonly options: SubagentActivityTrackerOptions) {}

  private publish(activity: SubagentActivityItem): void {
    void this.options.messageBus.publish({
      type: MessageBusType.SUBAGENT_ACTIVITY,
      subagentName: this.options.displayName ?? this.options.agentName,
      activity,
    });
  }

  private trimToLimit(): void {
    const limit = this.options.maxRecentActivity;
    if (limit !== undefined && this.recentActivity.length > limit) {
      this.recentActivity = this.recentActivity.slice(-limit);
    }
  }

  handleActivity(activity: SubagentActivityEvent): void {
    const { updateOutput, trimToolNames } = this.options;
    if (!updateOutput) {
      return;
    }

    let updated = false;

    switch (activity.type) {
      case 'THOUGHT_CHUNK': {
        const rawText = activity.data['text'];
        const text =
          typeof rawText === 'string'
            ? rawText
            : trimToolNames
              ? ''
              : String(rawText);
        const lastItem = this.recentActivity[this.recentActivity.length - 1];

        if (
          lastItem &&
          lastItem.type === 'thought' &&
          lastItem.status === SubagentState.RUNNING
        ) {
          lastItem.content = sanitizeThoughtContent(text);
        } else {
          this.recentActivity.push({
            id: randomUUID(),
            type: 'thought',
            content: sanitizeThoughtContent(text),
            status: SubagentState.RUNNING,
          });
        }
        updated = true;

        const latestThought =
          this.recentActivity[this.recentActivity.length - 1];
        if (latestThought) {
          this.publish(latestThought);
        }
        break;
      }
      case 'TOOL_CALL_START': {
        const rawName = activity.data['name'];
        const name = trimToolNames
          ? typeof rawName === 'string'
            ? rawName.trim()
            : ''
          : String(rawName);
        const rawDisplayName = activity.data['displayName'];
        const displayName = rawDisplayName
          ? sanitizeErrorMessage(
              trimToolNames
                ? String(rawDisplayName).trim()
                : String(rawDisplayName),
            )
          : undefined;
        const description = activity.data['description']
          ? sanitizeErrorMessage(String(activity.data['description']))
          : undefined;
        const args = JSON.stringify(sanitizeToolArgs(activity.data['args']));
        const callId = activity.data['callId']
          ? String(activity.data['callId'])
          : randomUUID();
        this.recentActivity.push({
          id: callId,
          type: 'tool_call',
          content: name,
          displayName,
          description,
          args,
          status: SubagentState.RUNNING,
        });
        updated = true;

        const latestTool = this.recentActivity[this.recentActivity.length - 1];
        if (latestTool) {
          this.publish(latestTool);
        }
        break;
      }
      case 'TOOL_CALL_END': {
        const data = activity.data['data'];
        const isError = isToolActivityError(data);
        const callId = activity.data['id']
          ? String(activity.data['id'])
          : undefined;

        if (callId) {
          for (let i = this.recentActivity.length - 1; i >= 0; i--) {
            if (
              this.recentActivity[i].type === 'tool_call' &&
              this.recentActivity[i].id === callId &&
              this.recentActivity[i].status === SubagentState.RUNNING
            ) {
              this.recentActivity[i].status = isError
                ? SubagentState.ERROR
                : SubagentState.COMPLETED;
              updated = true;
              this.publish(this.recentActivity[i]);
              break;
            }
          }
        }
        break;
      }
      case 'ERROR': {
        const rawError = activity.data['error'];
        const error = trimToolNames
          ? typeof rawError === 'string'
            ? rawError.trim()
            : ''
          : String(rawError);
        const errorType = activity.data['errorType'];
        const sanitizedError = sanitizeErrorMessage(error);
        const isCancellation =
          errorType === SubagentActivityErrorType.CANCELLED ||
          error === SUBAGENT_CANCELLED_ERROR_MESSAGE;
        const isRejection =
          errorType === SubagentActivityErrorType.REJECTED ||
          error.startsWith(SUBAGENT_REJECTED_ERROR_PREFIX);

        const callId = activity.data['callId']
          ? String(activity.data['callId'])
          : undefined;

        if (callId) {
          const targetStatus =
            isCancellation || isRejection
              ? SubagentState.CANCELLED
              : SubagentState.ERROR;

          for (let i = this.recentActivity.length - 1; i >= 0; i--) {
            if (
              this.recentActivity[i].type === 'tool_call' &&
              this.recentActivity[i].id === callId &&
              this.recentActivity[i].status === SubagentState.RUNNING
            ) {
              this.recentActivity[i].status = targetStatus;
              updated = true;
              break;
            }
          }
        }

        this.recentActivity.push({
          id: randomUUID(),
          type: 'thought',
          content:
            isCancellation || isRejection
              ? sanitizedError
              : `Error: ${sanitizedError}`,
          status:
            isCancellation || isRejection
              ? SubagentState.CANCELLED
              : SubagentState.ERROR,
        });
        updated = true;
        break;
      }
      default:
        break;
    }

    if (updated) {
      this.trimToLimit();
      updateOutput(this.buildProgress(SubagentState.RUNNING));
    }
  }

  finalizeError(error: unknown): {
    isAbort: boolean;
    errorMessage: string;
    progress: SubagentProgress;
  } {
    const errorMessage = error instanceof Error ? error.message : String(error);
    const isAbort =
      (error instanceof Error && error.name === 'AbortError') ||
      errorMessage.includes('Aborted');

    const targetStatus = isAbort
      ? SubagentState.CANCELLED
      : SubagentState.ERROR;
    for (const item of this.recentActivity) {
      if (item.status === SubagentState.RUNNING) {
        item.status = targetStatus;
      }
    }

    if (!isAbort) {
      const lastActivity = this.recentActivity[this.recentActivity.length - 1];
      if (!lastActivity || lastActivity.status !== SubagentState.ERROR) {
        this.recentActivity.push({
          id: randomUUID(),
          type: 'thought',
          content: `Error: ${errorMessage}`,
          status: SubagentState.ERROR,
        });
        this.trimToLimit();
      }
    }

    const progress = this.buildProgress(targetStatus);
    this.options.updateOutput?.(progress);
    return { isAbort, errorMessage, progress };
  }

  buildProgress(
    state: SubagentState,
    extra?: Pick<SubagentProgress, 'result' | 'terminateReason'>,
  ): SubagentProgress {
    return {
      isSubagentProgress: true,
      agentName: this.options.agentName,
      recentActivity: [...this.recentActivity],
      state,
      ...extra,
    };
  }
}

/**
 * Formats an execution error into a user-friendly message for remote agents.
 */
export function formatRemoteExecutionError(error: unknown): string {
  if (error instanceof A2AAgentError) {
    return error.userMessage;
  }

  return `Error calling remote agent: ${
    error instanceof Error ? error.message : String(error)
  }`;
}

/**
 * Returns a copy of activity items with any RUNNING entries transitioned to `status`.
 */
export function stopRunningActivities(
  activity: SubagentActivityItem[],
  status: SubagentState,
): SubagentActivityItem[] {
  return activity.map((item) =>
    item.status === SubagentState.RUNNING ? { ...item, status } : item,
  );
}
