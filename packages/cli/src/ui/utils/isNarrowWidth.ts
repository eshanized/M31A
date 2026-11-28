/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

export const NARROW_WIDTH_BREAKPOINT = 80;
export const COMPACT_WIDTH_BREAKPOINT = 60;
export const FULL_LABEL_WIDTH_BREAKPOINT = 100;
export const SIDEBAR_MIN_TERMINAL_WIDTH = 96;
export const SCREEN_READER_WIDTH_PERCENT = '90%';
export const SCREEN_READER_WIDTH_FRACTION = 0.9;

export function isNarrowWidth(width: number): boolean {
  return width < NARROW_WIDTH_BREAKPOINT;
}

export function isCompactWidth(width: number): boolean {
  return width < COMPACT_WIDTH_BREAKPOINT;
}

export function hasFullLabelWidth(width: number): boolean {
  return width >= FULL_LABEL_WIDTH_BREAKPOINT;
}

export function getScreenReaderContentWidth(width: number): number {
  return Math.max(20, Math.floor(width * SCREEN_READER_WIDTH_FRACTION));
}
