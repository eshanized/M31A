/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { logs, type LogRecord } from '@opentelemetry/api-logs';
import type { Config } from '../config/config.js';
import { SERVICE_NAME } from './constants.js';
import {
  EVENT_API_ERROR,
  EVENT_API_RESPONSE,
  EVENT_TOOL_CALL,
  EVENT_REWIND,
  type ApiErrorEvent,
  type ApiRequestEvent,
  type ApiResponseEvent,
  type FileOperationEvent,
  type IdeConnectionEvent,
  type StartSessionEvent,
  type ToolCallEvent,
  type UserPromptEvent,
  type FlashFallbackEvent,
  type NextSpeakerCheckEvent,
  type LoopDetectedEvent,
  type LoopDetectionDisabledEvent,
  type SlashCommandEvent,
  type RewindEvent,
  type ConversationFinishedEvent,
  type ChatCompressionEvent,
  type MalformedJsonResponseEvent,
  type InvalidChunkEvent,
  type ContentRetryEvent,
  type ContentRetryFailureEvent,
  type NetworkRetryAttemptEvent,
  type RipgrepFallbackEvent,
  type ToolOutputTruncatedEvent,
  type ModelRoutingEvent,
  type ExtensionDisableEvent,
  type ExtensionEnableEvent,
  type ExtensionUninstallEvent,
  type ExtensionInstallEvent,
  type ModelSlashCommandEvent,
  type EditStrategyEvent,
  type EditCorrectionEvent,
  type AgentStartEvent,
  type AgentFinishEvent,
  type RecoveryAttemptEvent,
  type WebFetchFallbackAttemptEvent,
  type ExtensionUpdateEvent,
  type ApprovalModeSwitchEvent,
  type ApprovalModeDurationEvent,
  type HookCallEvent,
  type StartupStatsEvent,
  type LlmLoopCheckEvent,
  type PlanExecutionEvent,
  type ToolOutputMaskingEvent,
  type KeychainAvailabilityEvent,
  type TokenStorageInitializationEvent,
} from './types.js';
import {
  recordApiErrorMetrics,
  recordToolCallMetrics,
  recordChatCompressionMetrics,
  recordFileOperationMetric,
  recordRetryAttemptMetrics,
  recordContentRetry,
  recordContentRetryFailure,
  recordModelRoutingMetrics,
  recordModelSlashCommand,
  getConventionAttributes,
  recordTokenUsageMetrics,
  recordApiResponseMetrics,
  recordAgentRunMetrics,
  recordRecoveryAttemptMetrics,
  recordLinesChanged,
  recordHookCallMetrics,
  recordPlanExecution,
  recordKeychainAvailability,
  recordTokenStorageInitialization,
  recordInvalidChunk,
  recordBrowserAgentConnection,
  recordBrowserAgentVisionStatus,
  recordBrowserAgentTaskOutcome,
  recordBrowserAgentCleanup,
} from './metrics.js';
import { bufferTelemetryEvent } from './sdk.js';
import { uiTelemetryService, type UiEvent } from './uiTelemetry.js';
import { debugLogger } from '../utils/debugLogger.js';

export function logCliConfiguration(
  config: Config,
  event: StartSessionEvent,
): void {
  bufferTelemetryEvent(() => {
    // Wait for experiments to load before emitting so we capture experimentIds
    void config
      .getExperimentsAsync()
      .then(() => {
        const logger = logs.getLogger(SERVICE_NAME);
        const logRecord: LogRecord = {
          body: event.toLogBody(),
          attributes: event.toOpenTelemetryAttributes(config),
        };
        logger.emit(logRecord);
      })
      .catch((e: unknown) => {
        debugLogger.error('Failed to log telemetry event', e);
      });
  }, config);
}

export function logUserPrompt(config: Config, event: UserPromptEvent): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);

    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logToolCall(config: Config, event: ToolCallEvent): void {
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion
  const uiEvent = {
    // eslint-disable-next-line @typescript-eslint/no-misused-spread
    ...event,
    'event.name': EVENT_TOOL_CALL,
    'event.timestamp': new Date().toISOString(),
  } as UiEvent;
  uiTelemetryService.addEvent(uiEvent);
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
    recordToolCallMetrics(config, event.duration_ms, {
      function_name: event.function_name,
      success: event.success,
      decision: event.decision,
      tool_type: event.tool_type,
    });

    if (event.metadata) {
      // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
      const added = event.metadata['model_added_lines'];
      if (typeof added === 'number' && added > 0) {
        recordLinesChanged(config, added, 'added', {
          function_name: event.function_name,
        });
      }
      // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
      const removed = event.metadata['model_removed_lines'];
      if (typeof removed === 'number' && removed > 0) {
        recordLinesChanged(config, removed, 'removed', {
          function_name: event.function_name,
        });
      }
    }
  }, config);
}

export function logToolOutputTruncated(
  config: Config,
  event: ToolOutputTruncatedEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logToolOutputMasking(
  config: Config,
  event: ToolOutputMaskingEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logFileOperation(
  config: Config,
  event: FileOperationEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);

    recordFileOperationMetric(config, {
      operation: event.operation,
      lines: event.lines,
      mimetype: event.mimetype,
      extension: event.extension,
      programming_language: event.programming_language,
    });
  }, config);
}

export function logApiRequest(config: Config, event: ApiRequestEvent): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    logger.emit(event.toLogRecord(config));
    logger.emit(event.toSemanticLogRecord(config));
  }, config);
}

export function logFlashFallback(
  config: Config,
  event: FlashFallbackEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logRipgrepFallback(
  config: Config,
  event: RipgrepFallbackEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logApiError(config: Config, event: ApiErrorEvent): void {
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion
  const uiEvent = {
    // eslint-disable-next-line @typescript-eslint/no-misused-spread
    ...event,
    'event.name': EVENT_API_ERROR,
    'event.timestamp': new Date().toISOString(),
  } as UiEvent;
  uiTelemetryService.addEvent(uiEvent);
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    logger.emit(event.toLogRecord(config));
    logger.emit(event.toSemanticLogRecord(config));

    recordApiErrorMetrics(config, event.duration_ms, {
      model: event.model,
      status_code: event.status_code,
      error_type: event.error_type,
    });

    // Record GenAI operation duration for errors
    recordApiResponseMetrics(config, event.duration_ms, {
      model: event.model,
      status_code: event.status_code,
      genAiAttributes: {
        ...getConventionAttributes(event),
        'error.type': event.error_type || 'unknown',
      },
    });
  }, config);
}

export function logApiResponse(config: Config, event: ApiResponseEvent): void {
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion
  const uiEvent = {
    // eslint-disable-next-line @typescript-eslint/no-misused-spread
    ...event,
    'event.name': EVENT_API_RESPONSE,
    'event.timestamp': new Date().toISOString(),
  } as UiEvent;
  uiTelemetryService.addEvent(uiEvent);
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    logger.emit(event.toLogRecord(config));
    logger.emit(event.toSemanticLogRecord(config));

    const conventionAttributes = getConventionAttributes(event);

    recordApiResponseMetrics(config, event.duration_ms, {
      model: event.model,
      status_code: event.status_code,
      genAiAttributes: conventionAttributes,
    });

    const tokenUsageData = [
      { count: event.usage.input_token_count, type: 'input' as const },
      { count: event.usage.output_token_count, type: 'output' as const },
      { count: event.usage.cached_content_token_count, type: 'cache' as const },
      { count: event.usage.thoughts_token_count, type: 'thought' as const },
      { count: event.usage.tool_token_count, type: 'tool' as const },
    ];

    for (const { count, type } of tokenUsageData) {
      recordTokenUsageMetrics(config, count, {
        model: event.model,
        type,
        genAiAttributes: conventionAttributes,
      });
    }
  }, config);
}

export function logLoopDetected(
  config: Config,
  event: LoopDetectedEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logLoopDetectionDisabled(
  config: Config,
  event: LoopDetectionDisabledEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logNextSpeakerCheck(
  config: Config,
  event: NextSpeakerCheckEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logSlashCommand(
  config: Config,
  event: SlashCommandEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logRewind(config: Config, event: RewindEvent): void {
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion
  const uiEvent = {
    // eslint-disable-next-line @typescript-eslint/no-misused-spread
    ...event,
    'event.name': EVENT_REWIND,
    'event.timestamp': new Date().toISOString(),
  } as UiEvent;
  uiTelemetryService.addEvent(uiEvent);
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logIdeConnection(
  config: Config,
  event: IdeConnectionEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logConversationFinishedEvent(
  config: Config,
  event: ConversationFinishedEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logChatCompression(
  config: Config,
  event: ChatCompressionEvent,
): void {
  const logger = logs.getLogger(SERVICE_NAME);
  const logRecord: LogRecord = {
    body: event.toLogBody(),
    attributes: event.toOpenTelemetryAttributes(config),
  };
  logger.emit(logRecord);

  recordChatCompressionMetrics(config, {
    tokens_before: event.tokens_before,
    tokens_after: event.tokens_after,
  });
}

export function logMalformedJsonResponse(
  config: Config,
  event: MalformedJsonResponseEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logInvalidChunk(
  config: Config,
  event: InvalidChunkEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
    recordInvalidChunk(config);
  }, config);
}

export function logNetworkRetryAttempt(
  config: Config,
  event: NetworkRetryAttemptEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
    recordRetryAttemptMetrics(config, {
      model: event.model,
      attempt: event.attempt,
    });
  }, config);
}

export function logContentRetry(
  config: Config,
  event: ContentRetryEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
    recordContentRetry(config);
  }, config);
}

export function logContentRetryFailure(
  config: Config,
  event: ContentRetryFailureEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
    recordContentRetryFailure(config);
  }, config);
}

export function logModelRouting(
  config: Config,
  event: ModelRoutingEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
    recordModelRoutingMetrics(config, event);
  }, config);
}

export function logModelSlashCommand(
  config: Config,
  event: ModelSlashCommandEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
    recordModelSlashCommand(config, event);
  }, config);
}

export async function logExtensionInstallEvent(
  config: Config,
  event: ExtensionInstallEvent,
): Promise<void> {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export async function logExtensionUninstall(
  config: Config,
  event: ExtensionUninstallEvent,
): Promise<void> {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export async function logExtensionUpdateEvent(
  config: Config,
  event: ExtensionUpdateEvent,
): Promise<void> {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export async function logExtensionEnable(
  config: Config,
  event: ExtensionEnableEvent,
): Promise<void> {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export async function logExtensionDisable(
  config: Config,
  event: ExtensionDisableEvent,
): Promise<void> {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logEditStrategy(
  config: Config,
  event: EditStrategyEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logEditCorrectionEvent(
  config: Config,
  event: EditCorrectionEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logAgentStart(config: Config, event: AgentStartEvent): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logAgentFinish(config: Config, event: AgentFinishEvent): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);

    recordAgentRunMetrics(config, event);
  }, config);
}

export function logRecoveryAttempt(
  config: Config,
  event: RecoveryAttemptEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);

    recordRecoveryAttemptMetrics(config, event);
  }, config);
}

export function logWebFetchFallbackAttempt(
  config: Config,
  event: WebFetchFallbackAttemptEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logLlmLoopCheck(
  config: Config,
  event: LlmLoopCheckEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);
  }, config);
}

export function logApprovalModeSwitch(
  config: Config,
  event: ApprovalModeSwitchEvent,
) {
  bufferTelemetryEvent(() => {
    logs.getLogger(SERVICE_NAME).emit({
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    });
  }, config);
}

export function logApprovalModeDuration(
  config: Config,
  event: ApprovalModeDurationEvent,
) {
  bufferTelemetryEvent(() => {
    logs.getLogger(SERVICE_NAME).emit({
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    });
  }, config);
}

export function logPlanExecution(config: Config, event: PlanExecutionEvent) {
  bufferTelemetryEvent(() => {
    logs.getLogger(SERVICE_NAME).emit({
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    });

    recordPlanExecution(config, {
      approval_mode: event.approval_mode,
    });
  }, config);
}

export function logHookCall(config: Config, event: HookCallEvent): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);

    recordHookCallMetrics(
      config,
      event.hook_event_name,
      event.hook_name,
      event.duration_ms,
      event.success,
    );
  }, config);
}

export function logStartupStats(
  config: Config,
  event: StartupStatsEvent,
): void {
  bufferTelemetryEvent(() => {
    // Wait for experiments to load before emitting so we capture experimentIds
    void config
      .getExperimentsAsync()
      .then(() => {
        const logger = logs.getLogger(SERVICE_NAME);
        const logRecord: LogRecord = {
          body: event.toLogBody(),
          attributes: event.toOpenTelemetryAttributes(config),
        };
        logger.emit(logRecord);
      })
      .catch((e: unknown) => {
        debugLogger.error('Failed to log telemetry event', e);
      });
  }, config);
}

export function logKeychainAvailability(
  config: Config,
  event: KeychainAvailabilityEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);

    recordKeychainAvailability(config, event);
  }, config);
}

export function logTokenStorageInitialization(
  config: Config,
  event: TokenStorageInitializationEvent,
): void {
  bufferTelemetryEvent(() => {
    const logger = logs.getLogger(SERVICE_NAME);
    const logRecord: LogRecord = {
      body: event.toLogBody(),
      attributes: event.toOpenTelemetryAttributes(config),
    };
    logger.emit(logRecord);

    recordTokenStorageInitialization(config, event);
  }, config);
}

// ==========================================================================
// Browser Agent Events
// ==========================================================================

export function logBrowserAgentConnection(
  config: Config,
  durationMs: number,
  attributes: {
    session_mode: 'persistent' | 'isolated' | 'existing';
    headless: boolean;
    success: boolean;
    error_type?:
      | 'profile_locked'
      | 'timeout'
      | 'connection_refused'
      | 'unknown';
    tool_count?: number;
  },
): void {
  recordBrowserAgentConnection(config, durationMs, attributes);
}

export function logBrowserAgentVisionStatus(
  config: Config,
  attributes: {
    enabled: boolean;
    disabled_reason?: 'no_visual_model' | 'missing_visual_tools';
  },
): void {
  recordBrowserAgentVisionStatus(config, attributes);
}

export function logBrowserAgentTaskOutcome(
  config: Config,
  attributes: {
    success: boolean;
    session_mode: 'persistent' | 'isolated' | 'existing';
    vision_enabled: boolean;
    headless: boolean;
    duration_ms: number;
  },
): void {
  recordBrowserAgentTaskOutcome(config, attributes);
}

export function logBrowserAgentCleanup(
  config: Config,
  durationMs: number,
  attributes: {
    session_mode: 'persistent' | 'isolated' | 'existing';
    success: boolean;
  },
): void {
  recordBrowserAgentCleanup(config, durationMs, attributes);
}
