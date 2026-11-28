/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type { Config } from '@m31a/core';
import { isAlternateBufferEnabled } from '../hooks/useAlternateBuffer.js';
import { SIDEBAR_MIN_TERMINAL_WIDTH } from './isNarrowWidth.js';

export { SIDEBAR_MIN_TERMINAL_WIDTH };

export const calculateMainAreaWidth = (
  terminalWidth: number,
  config: Config,
): number => {
  if (isAlternateBufferEnabled(config)) {
    return terminalWidth - 1;
  }
  return terminalWidth;
};

export const calculateSidebarWidth = (
  terminalWidth: number,
  showSidebar: boolean,
): number => {
  if (!showSidebar || terminalWidth < SIDEBAR_MIN_TERMINAL_WIDTH) {
    return 0;
  }
  return Math.min(34, Math.max(26, Math.floor(terminalWidth * 0.25)));
};

export const calculateContentAreaWidth = (
  terminalWidth: number,
  config: Config,
  sidebarWidth: number,
): number => {
  const baseWidth = calculateMainAreaWidth(terminalWidth, config);
  if (sidebarWidth <= 0) {
    return baseWidth;
  }
  return Math.max(20, baseWidth - sidebarWidth - 1);
};
