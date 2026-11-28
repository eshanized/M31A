/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useEffect, useState } from 'react';
import { Text, useIsScreenReaderEnabled } from 'ink';
import type { SpinnerName } from 'cli-spinners';
import { isTmux } from '@m31a/core';
import tinygradient from 'tinygradient';
import { CliSpinner, TMUX_SPINNER_INTERVAL_MS } from './CliSpinner.js';
import { theme } from '../semantic-colors.js';
import { SCREEN_READER_LOADING } from '../textConstants.js';

const COLOR_CYCLE_DURATION_MS = 4000;
const DEFAULT_COLOR_CYCLE_INTERVAL_MS = 120;

export function isReducedMotionEnabled(): boolean {
  const v = (process.env['M31A_REDUCED_MOTION'] ?? '').toLowerCase();
  return v === '1' || v === 'true' || v === 'yes';
}

export interface StatusGlyphProps {
  /** Whether the active spinner state is shown. Defaults to true. */
  active?: boolean;
  /** Static glyph/string shown when `active` is false (or hook overrides). */
  nonRespondingDisplay?: string;
  /** Spinner animation type when `active` is true. */
  spinnerType?: SpinnerName;
  /** Color mode: 'gradient' cycles active theme tokens; 'mono' uses a single color. */
  variant?: 'gradient' | 'mono';
  /** Explicit color for mono/non-responding state. */
  color?: string;
  /** Screen-reader text when `active` is true. */
  altText?: string;
  /** Screen-reader text when `nonRespondingDisplay` is shown. */
  nonRespondingAltText?: string;
  /** Override motion reduction; defaults to M31A_REDUCED_MOTION / screen-reader. */
  reducedMotion?: boolean;
}

/**
 * Unified status glyph / spinner component with theme binding, tmux throttling,
 * M31A_REDUCED_MOTION support, and screen-reader altText fallback.
 */
export const StatusGlyph: React.FC<StatusGlyphProps> = React.memo(
  ({
    active = true,
    nonRespondingDisplay,
    spinnerType = 'dots',
    variant = 'mono',
    color,
    altText,
    nonRespondingAltText = SCREEN_READER_LOADING,
    reducedMotion,
  }: StatusGlyphProps) => {
    const isScreenReaderEnabled = useIsScreenReaderEnabled();
    const [time, setTime] = useState(0);

    const isReducedMotion =
      reducedMotion ?? (isScreenReaderEnabled || isReducedMotionEnabled());
    const intervalMs = isTmux()
      ? TMUX_SPINNER_INTERVAL_MS
      : DEFAULT_COLOR_CYCLE_INTERVAL_MS;

    const shouldCycleGradient =
      active &&
      variant === 'gradient' &&
      !isScreenReaderEnabled &&
      !isReducedMotion;

    useEffect(() => {
      if (!shouldCycleGradient) {
        return;
      }

      const interval = setInterval(() => {
        setTime((prevTime) => prevTime + intervalMs);
      }, intervalMs);

      return () => clearInterval(interval);
    }, [shouldCycleGradient, intervalMs]);

    if (!active) {
      if (!nonRespondingDisplay) {
        return null;
      }
      if (isScreenReaderEnabled) {
        return <Text>{nonRespondingAltText}</Text>;
      }
      return (
        <Text color={color ?? theme.text.primary}>{nonRespondingDisplay}</Text>
      );
    }

    if (isScreenReaderEnabled) {
      return <Text>{altText}</Text>;
    }

    if (isReducedMotion || variant === 'mono') {
      return (
        <Text color={color ?? theme.text.accent ?? theme.text.primary}>
          <CliSpinner type={spinnerType} reducedMotion={isReducedMotion} />
        </Text>
      );
    }

    const g = theme.ui.gradient;
    const colors =
      g && g.length > 0
        ? g
        : [
            theme.text.accent,
            theme.ui.active,
            theme.ui.focus,
            theme.text.primary,
          ];
    const clean = colors.filter((c): c is string => !!c);
    const palette = clean.length > 0 ? clean : [theme.text.primary];
    const gradient = tinygradient([...palette, palette[0]]);
    const progress = (time % COLOR_CYCLE_DURATION_MS) / COLOR_CYCLE_DURATION_MS;
    const currentColor = gradient.rgbAt(progress).toHexString();

    return (
      <Text color={currentColor}>
        <CliSpinner type={spinnerType} reducedMotion={false} />
      </Text>
    );
  },
);
StatusGlyph.displayName = 'StatusGlyph';
