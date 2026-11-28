/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import type { SpinnerName } from 'cli-spinners';
import { StatusGlyph } from './StatusGlyph.js';

interface M31aSpinnerProps {
  spinnerType?: SpinnerName;
  altText?: string;
  /** Override motion reduction; defaults to M31A_REDUCED_MOTION / screen-reader. */
  reducedMotion?: boolean;
}

export const M31aSpinner: React.FC<M31aSpinnerProps> = React.memo(
  ({ spinnerType = 'dots', altText, reducedMotion }: M31aSpinnerProps) => (
    <StatusGlyph
      active={true}
      variant="gradient"
      spinnerType={spinnerType}
      altText={altText}
      reducedMotion={reducedMotion}
    />
  ),
);
M31aSpinner.displayName = 'M31aSpinner';
