/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { afterEach, describe, expect, it } from 'vitest';
import {
  DEFAULT_CONTEXT_FILENAME,
  getAllM31aMdFilenames,
  resetM31aMdFilename,
  setM31aMdFilename,
} from './memoryTool.js';

describe('memoryTool filename helpers', () => {
  afterEach(() => {
    resetM31aMdFilename(DEFAULT_CONTEXT_FILENAME);
  });

  describe('setM31aMdFilename', () => {
    it('appends to currentGeminiMdFilename when a valid new name is provided', () => {
      const newName = 'CUSTOM_CONTEXT.md';
      setM31aMdFilename(newName);
      expect(getAllM31aMdFilenames()).toEqual([
        newName,
        DEFAULT_CONTEXT_FILENAME,
      ]);
    });

    it('does not update currentGeminiMdFilename if the new name is empty or whitespace', () => {
      const initialNames = getAllM31aMdFilenames();
      setM31aMdFilename('  ');
      expect(getAllM31aMdFilenames()).toEqual(initialNames);

      setM31aMdFilename('');
      expect(getAllM31aMdFilenames()).toEqual(initialNames);
    });

    it('handles adding an array of filenames', () => {
      const newNames = ['CUSTOM_CONTEXT.md', 'ANOTHER_CONTEXT.md'];
      setM31aMdFilename(newNames);
      expect(getAllM31aMdFilenames()).toEqual([
        ...newNames,
        DEFAULT_CONTEXT_FILENAME,
      ]);
    });

    it('ensures uniqueness when adding names', () => {
      setM31aMdFilename(DEFAULT_CONTEXT_FILENAME);
      expect(getAllM31aMdFilenames()).toEqual([DEFAULT_CONTEXT_FILENAME]);

      setM31aMdFilename(['NEW.md', 'NEW.md']);
      expect(getAllM31aMdFilenames()).toEqual([
        'NEW.md',
        DEFAULT_CONTEXT_FILENAME,
      ]);
    });
  });

  describe('resetM31aMdFilename', () => {
    it('replaces all filenames with the provided one', () => {
      setM31aMdFilename('OTHER.md');
      resetM31aMdFilename('RESET.md');
      expect(getAllM31aMdFilenames()).toEqual(['RESET.md']);
    });

    it('resets to default if no argument provided', () => {
      resetM31aMdFilename('OTHER.md');
      resetM31aMdFilename(DEFAULT_CONTEXT_FILENAME);
      expect(getAllM31aMdFilenames()).toEqual([DEFAULT_CONTEXT_FILENAME]);
    });

    it('handles array reset', () => {
      resetM31aMdFilename(['A.md', 'B.md']);
      expect(getAllM31aMdFilenames()).toEqual(['A.md', 'B.md']);
    });
  });
});
