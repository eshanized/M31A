/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import type { SpinnerName } from 'cli-spinners';
import { useStreamingContext } from '../contexts/StreamingContext.js';
import { StreamingState } from '../types.js';
import {
  SCREEN_READER_LOADING,
  SCREEN_READER_RESPONDING,
} from '../textConstants.js';
import { M31aSpinner } from './M31aSpinner.js';
import { StatusGlyph } from './StatusGlyph.js';

interface M31aRespondingSpinnerProps {
  /**
   * Optional string to display when not in Responding state.
   * If not provided and not Responding, renders null.
   */
  nonRespondingDisplay?: string;
  spinnerType?: SpinnerName;
  /**
   * If true, we prioritize showing the nonRespondingDisplay (hook icon)
   * even if the state is Responding.
   */
  isHookActive?: boolean;
  color?: string;
}

export const M31aRespondingSpinner: React.FC<M31aRespondingSpinnerProps> =
  React.memo(
    ({
      nonRespondingDisplay,
      spinnerType = 'dots',
      isHookActive = false,
      color,
    }: M31aRespondingSpinnerProps) => {
      const streamingState = useStreamingContext();

      // If a hook is active, keep showing the hook icon (nonRespondingDisplay)
      // consistently instead of the active response spinner.
      if (streamingState === StreamingState.Responding && !isHookActive) {
        return (
          <M31aSpinner
            spinnerType={spinnerType}
            altText={SCREEN_READER_RESPONDING}
          />
        );
      }

      return (
        <StatusGlyph
          active={false}
          nonRespondingDisplay={nonRespondingDisplay}
          nonRespondingAltText={SCREEN_READER_LOADING}
          color={color}
        />
      );
    },
  );
M31aRespondingSpinner.displayName = 'M31aRespondingSpinner';
