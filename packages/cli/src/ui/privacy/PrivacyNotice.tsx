/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { Box, Newline, Text } from 'ink';
import { type Config } from '@m31a/core';
import { theme } from '../semantic-colors.js';
import { useKeypress } from '../hooks/useKeypress.js';

interface PrivacyNoticeProps {
  onExit: () => void;
  config: Config;
}

const PrivacyNoticeText = ({ onExit }: { onExit: () => void }) => {
  useKeypress(
    (key) => {
      if (key.name === 'escape' || key.name === 'return') {
        onExit();
        return true;
      }
      return false;
    },
    { isActive: true },
  );

  return (
    <Box flexDirection="column" marginBottom={1}>
      <Text bold color={theme.text.accent}>
        M31A Privacy Notice
      </Text>
      <Newline />
      <Text color={theme.text.primary}>
        M31A sends your prompts and tool inputs directly to the configured
        NVIDIA NIM endpoint using your NVIDIA_API_KEY. Optional OpenTelemetry
        (OTLP) export is disabled unless configured.
      </Text>
      <Newline />
      <Text color={theme.text.secondary}>Press Esc to exit.</Text>
    </Box>
  );
};

export const PrivacyNotice = ({
  onExit,
  config: _config,
}: PrivacyNoticeProps) => (
  <Box borderStyle="round" padding={1} flexDirection="column">
    <PrivacyNoticeText onExit={onExit} />
  </Box>
);
