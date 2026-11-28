/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { Text } from 'ink';
import { theme } from '../semantic-colors.js';
import { getContextUsagePercentage } from '../utils/contextUsage.js';
import { useSettings } from '../contexts/SettingsContext.js';
import { DEFAULT_COMPRESSION_THRESHOLD } from '../constants.js';
import { hasFullLabelWidth } from '../utils/isNarrowWidth.js';

export const ContextUsageDisplay = ({
  promptTokenCount,
  model,
  terminalWidth,
}: {
  promptTokenCount: number;
  model: string | undefined;
  terminalWidth: number;
}) => {
  const settings = useSettings();
  const contextWindowOverride = settings.merged.model?.contextWindow;
  const percentage = getContextUsagePercentage(
    promptTokenCount,
    model,
    contextWindowOverride,
  );
  const percentageUsed = (percentage * 100).toFixed(0);

  const threshold =
    settings.merged.model?.compressionThreshold ??
    DEFAULT_COMPRESSION_THRESHOLD;

  let textColor = theme.text.secondary;
  if (percentage >= 1.0) {
    textColor = theme.status.error;
  } else if (percentage >= threshold) {
    textColor = theme.status.warning;
  }

  const label = hasFullLabelWidth(terminalWidth) ? '% used' : '%';

  return (
    <Text color={textColor}>
      {percentageUsed}
      {label}
    </Text>
  );
};
