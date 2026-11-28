/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useState, useEffect } from 'react';
import { Text, useIsScreenReaderEnabled } from 'ink';
import { isTmux } from '@m31a/core';
import { isReducedMotionEnabled } from './StatusGlyph.js';
import { TMUX_SPINNER_INTERVAL_MS } from './CliSpinner.js';

const WAVE_CHARS = [' ', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
const ANIMATION_SPEED = 0.4;
const BAR_PHASE_OFFSET = 1.2;
const MAX_HEIGHT_MULTIPLIER = 3.5;
const FRAME_INTERVAL_MS = 120;

export interface ListeningIndicatorProps {
  color?: string;
}

export const ListeningIndicator: React.FC<ListeningIndicatorProps> = React.memo(
  ({ color }: ListeningIndicatorProps) => {
    const [tick, setTick] = useState(0);
    const isScreenReaderEnabled = useIsScreenReaderEnabled();
    const isReducedMotion = isScreenReaderEnabled || isReducedMotionEnabled();
    const intervalMs = isTmux()
      ? Math.max(FRAME_INTERVAL_MS, TMUX_SPINNER_INTERVAL_MS)
      : FRAME_INTERVAL_MS;

    useEffect(() => {
      if (isReducedMotion) return;
      const timer = setInterval(() => setTick((t) => t + 1), intervalMs);
      return () => clearInterval(timer);
    }, [isReducedMotion, intervalMs]);

    if (isReducedMotion) {
      return <Text color={color}>Listening... </Text>;
    }

    // Generate 3 bars for the wave
    const bars = Array.from({ length: 3 }).map((_, i) => {
      // Sine wave calculation to map to our 8 block characters (0-7)
      const phase = tick * ANIMATION_SPEED + i * BAR_PHASE_OFFSET;
      const height = Math.floor((Math.sin(phase) + 1) * MAX_HEIGHT_MULTIPLIER);
      return WAVE_CHARS[Math.max(0, Math.min(7, height))] ?? ' ';
    });

    return <Text color={color}>{bars.join('')} </Text>;
  },
);
ListeningIndicator.displayName = 'ListeningIndicator';
