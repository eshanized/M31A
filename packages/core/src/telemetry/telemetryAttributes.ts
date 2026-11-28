/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type { Attributes } from '@opentelemetry/api';
import type { Config } from '../config/config.js';
import { InstallationManager } from '../utils/installationManager.js';

const installationManager = new InstallationManager();

export function getCommonAttributes(config: Config): Attributes {
  const experiments = config.getExperiments();
  const authType = config.getContentGeneratorConfig()?.authType;
  return {
    'session.id': config.getSessionId(),
    'installation.id': installationManager.getInstallationId(),
    interactive: config.isInteractive(),
    ...(authType && { auth_type: authType }),
    ...(experiments &&
      experiments.experimentIds.length > 0 && {
        'experiments.ids': experiments.experimentIds,
      }),
  };
}
