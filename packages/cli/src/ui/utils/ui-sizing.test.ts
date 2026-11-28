/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect } from 'vitest';
import {
  calculateContentAreaWidth,
  calculateMainAreaWidth,
  calculateSidebarWidth,
} from './ui-sizing.js';
import type { Config } from '@m31a/core';

describe('ui-sizing', () => {
  describe('calculateMainAreaWidth', () => {
    it.each([
      // expected, width, altBuffer
      [80, 80, false],
      [100, 100, false],
      [79, 80, true],
      [99, 100, true],
    ])(
      'should return %i when width=%i and altBuffer=%s',
      (expected, width, altBuffer) => {
        const mockConfig = {
          getUseAlternateBuffer: () => altBuffer,
          getUseTerminalBuffer: () => false,
        } as unknown as Config;
        expect(calculateMainAreaWidth(width, mockConfig)).toBe(expected);
      },
    );
  });

  describe('calculateSidebarWidth & calculateContentAreaWidth', () => {
    it('hides sidebar below minimum width or when disabled', () => {
      expect(calculateSidebarWidth(80, true)).toBe(0);
      expect(calculateSidebarWidth(120, false)).toBe(0);
    });

    it('allocates clamped sidebar width on wide terminals', () => {
      expect(calculateSidebarWidth(100, true)).toBe(26);
      expect(calculateSidebarWidth(120, true)).toBe(30);
      expect(calculateSidebarWidth(180, true)).toBe(34);
    });

    it('subtracts sidebar and divider gap from content area width', () => {
      const mockConfig = {
        getUseAlternateBuffer: () => true,
        getUseTerminalBuffer: () => false,
      } as unknown as Config;
      const sidebar = calculateSidebarWidth(120, true);
      expect(sidebar).toBe(30);
      expect(calculateContentAreaWidth(120, mockConfig, sidebar)).toBe(88);
    });
  });
});
