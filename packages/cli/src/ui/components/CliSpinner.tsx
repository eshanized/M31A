/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import Spinner from 'ink-spinner';
import cliSpinners, { type SpinnerName } from 'cli-spinners';
import { Text, useIsScreenReaderEnabled } from 'ink';
import React, {
  type ComponentProps,
  useContext,
  useEffect,
  useState,
} from 'react';
import { isTmux } from '@m31a/core';
import { debugState } from '../debug.js';
import { SettingsContext } from '../contexts/SettingsContext.js';

export type SpinnerProps = ComponentProps<typeof Spinner> & {
  altText?: string;
  reducedMotion?: boolean;
};

export const TMUX_SPINNER_INTERVAL_MS = 120;

function envReducedMotion(): boolean {
  const v = (process.env['M31A_REDUCED_MOTION'] ?? '').toLowerCase();
  return v === '1' || v === 'true' || v === 'yes';
}

const TmuxSpinner: React.FC<{ type?: SpinnerName }> = React.memo(
  ({ type = 'dots' }: { type?: SpinnerName }) => {
    const [frame, setFrame] = useState(0);
    const spinner = cliSpinners[type] ?? cliSpinners.dots;
    const intervalMs = Math.max(spinner.interval, TMUX_SPINNER_INTERVAL_MS);

    useEffect(() => {
      const timer = setInterval(() => {
        setFrame((previousFrame) =>
          previousFrame >= spinner.frames.length - 1 ? 0 : previousFrame + 1,
        );
      }, intervalMs);

      return () => {
        clearInterval(timer);
      };
    }, [spinner, intervalMs]);

    return <Text>{spinner.frames[frame % spinner.frames.length]}</Text>;
  },
);
TmuxSpinner.displayName = 'TmuxSpinner';

export const CliSpinner: React.FC<SpinnerProps> = React.memo(
  ({ altText, reducedMotion, ...props }: SpinnerProps) => {
    const settings = useContext(SettingsContext);
    const isScreenReaderEnabled = useIsScreenReaderEnabled();
    const shouldShow = settings?.merged.ui?.showSpinner !== false;
    const inTmux = isTmux();
    const isReduced =
      reducedMotion ?? (isScreenReaderEnabled || envReducedMotion());

    useEffect(() => {
      if (shouldShow && !isReduced && !isScreenReaderEnabled) {
        debugState.debugNumAnimatedComponents++;
        return () => {
          debugState.debugNumAnimatedComponents--;
        };
      }
      return undefined;
    }, [shouldShow, isReduced, isScreenReaderEnabled]);

    if (!shouldShow) {
      return null;
    }

    if (isScreenReaderEnabled) {
      return altText ? <Text>{altText}</Text> : null;
    }

    if (isReduced) {
      const spinner = cliSpinners[props.type ?? 'dots'] ?? cliSpinners.dots;
      return <Text>{spinner.frames[0]}</Text>;
    }

    if (inTmux) {
      return <TmuxSpinner type={props.type} />;
    }

    return <Spinner {...props} />;
  },
);
CliSpinner.displayName = 'CliSpinner';
