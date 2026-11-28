/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { useMemo } from 'react';
import { Box, Text } from 'ink';
import { theme } from '../semantic-colors.js';
import { AuthType, type Config } from '@m31a/core';
import { hasNvidiaNimKey } from './statusColumns.js';

interface UserIdentityProps {
  config: Config;
}

export interface ProviderSummary {
  authType: AuthType;
  label: string;
  isWarning: boolean;
  tierName: string | undefined;
}

export function getProviderSummary(config: Config): ProviderSummary | null {
  const authType = config.getContentGeneratorConfig()?.authType;
  if (!authType) {
    return null;
  }

  const isNim = authType === AuthType.USE_NVIDIA_NIM;
  const hasKey = isNim ? hasNvidiaNimKey(config) : true;
  return {
    authType,
    label: isNim ? `NVIDIA NIM (${hasKey ? 'key ✓' : 'no key'})` : authType,
    isWarning: isNim && !hasKey,
    tierName: config.getUserTierName(),
  };
}

export const UserIdentity: React.FC<UserIdentityProps> = ({ config }) => {
  const summary = useMemo(() => getProviderSummary(config), [config]);

  if (!summary) {
    return null;
  }

  return (
    <Box flexDirection="column">
      <Box>
        <Text color={theme.text.secondary}>Provider: </Text>
        <Text
          color={
            summary.isWarning ? theme.status.warning : theme.status.success
          }
          wrap="truncate-end"
        >
          {summary.label}
        </Text>
      </Box>

      {summary.tierName && (
        <Box>
          <Text color={theme.text.primary} wrap="truncate-end">
            <Text bold>Plan:</Text> {summary.tierName}
          </Text>
        </Box>
      )}
    </Box>
  );
};
