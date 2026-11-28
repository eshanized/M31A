/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect } from 'vitest';
import {
  isNarrowWidth,
  isCompactWidth,
  hasFullLabelWidth,
  getScreenReaderContentWidth,
  NARROW_WIDTH_BREAKPOINT,
  COMPACT_WIDTH_BREAKPOINT,
  FULL_LABEL_WIDTH_BREAKPOINT,
} from './isNarrowWidth.js';

describe('isNarrowWidth breakpoints', () => {
  it('detects narrow widths below 80 columns', () => {
    expect(isNarrowWidth(NARROW_WIDTH_BREAKPOINT - 1)).toBe(true);
    expect(isNarrowWidth(NARROW_WIDTH_BREAKPOINT)).toBe(false);
  });

  it('detects compact widths below 60 columns', () => {
    expect(isCompactWidth(COMPACT_WIDTH_BREAKPOINT - 1)).toBe(true);
    expect(isCompactWidth(COMPACT_WIDTH_BREAKPOINT)).toBe(false);
  });

  it('detects full label widths at or above 100 columns', () => {
    expect(hasFullLabelWidth(FULL_LABEL_WIDTH_BREAKPOINT - 1)).toBe(false);
    expect(hasFullLabelWidth(FULL_LABEL_WIDTH_BREAKPOINT)).toBe(true);
  });

  it('computes 90% screen reader content width clamped to at least 20', () => {
    expect(getScreenReaderContentWidth(100)).toBe(90);
    expect(getScreenReaderContentWidth(80)).toBe(72);
    expect(getScreenReaderContentWidth(10)).toBe(20);
  });
});
