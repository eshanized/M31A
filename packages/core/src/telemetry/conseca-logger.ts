/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { logs, type LogRecord } from '@opentelemetry/api-logs';
import type { Config } from '../config/config.js';
import { SERVICE_NAME } from './constants.js';
import { isTelemetrySdkInitialized } from './sdk.js';
import type {
  ConsecaPolicyGenerationEvent,
  ConsecaVerdictEvent,
} from './types.js';
import { debugLogger } from '../utils/debugLogger.js';

export function logConsecaPolicyGeneration(
  config: Config,
  event: ConsecaPolicyGenerationEvent,
): void {
  debugLogger.debug('Conseca Policy Generation Event:', event);

  if (!isTelemetrySdkInitialized()) return;

  const logger = logs.getLogger(SERVICE_NAME);
  const logRecord: LogRecord = {
    body: event.toLogBody(),
    attributes: event.toOpenTelemetryAttributes(config),
  };
  logger.emit(logRecord);
}

export function logConsecaVerdict(
  config: Config,
  event: ConsecaVerdictEvent,
): void {
  debugLogger.debug('Conseca Verdict Event:', event);

  if (!isTelemetrySdkInitialized()) return;

  const logger = logs.getLogger(SERVICE_NAME);
  const logRecord: LogRecord = {
    body: event.toLogBody(),
    attributes: event.toOpenTelemetryAttributes(config),
  };
  logger.emit(logRecord);
}
