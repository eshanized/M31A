/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect } from 'vitest';
import {
  computeTailHeal,
  healStreamingInline,
  splitPendingMarker,
  healStreamingMarkdownDocument,
} from './markdownTailHeal.js';

describe('markdownTailHeal', () => {
  describe('healStreamingInline', () => {
    it('closes open bold and italic spans', () => {
      expect(healStreamingInline('Hello **world')).toBe('Hello **world**');
      expect(healStreamingInline('Hello *world')).toBe('Hello *world*');
      expect(healStreamingInline('Hello ~~gone')).toBe('Hello ~~gone~~');
      expect(healStreamingInline('Run `npm test')).toBe('Run `npm test`');
    });

    it('hides dangling opener markers with no content after them', () => {
      expect(healStreamingInline('Hello **')).toBe('Hello');
      expect(healStreamingInline('Run `')).toBe('Run');
    });

    it('heals unfinished markdown links and images cleanly', () => {
      expect(healStreamingInline('See [the docs](https://example.com')).toBe(
        'See the docs',
      );
      expect(healStreamingInline('Image ![alt text](https://img.png')).toBe(
        'Image',
      );
    });

    it('leaves balanced inline markdown untouched', () => {
      expect(healStreamingInline('**bold** and `code`')).toBe(
        '**bold** and `code`',
      );
      expect(computeTailHeal('Normal sentence.')).toEqual({
        strip: 0,
        append: '',
      });
    });
  });

  describe('splitPendingMarker', () => {
    it('holds bare heading, list, quote, and fence markers until content arrives', () => {
      expect(splitPendingMarker('Done.\n##')).toEqual({
        visible: 'Done.\n',
        held: '##',
      });
      expect(splitPendingMarker('Done.\n-')).toEqual({
        visible: 'Done.\n',
        held: '-',
      });
      expect(splitPendingMarker('Done.\n12.')).toEqual({
        visible: 'Done.\n',
        held: '12.',
      });
      expect(splitPendingMarker('Done.\n```')).toEqual({
        visible: 'Done.\n',
        held: '```',
      });
    });

    it('releases marker once content follows on the line', () => {
      expect(splitPendingMarker('Done.\n## Heading')).toEqual({
        visible: 'Done.\n## Heading',
        held: '',
      });
    });
  });

  describe('healStreamingMarkdownDocument', () => {
    it('heals trailing paragraph and heading content without affecting completed lines', () => {
      expect(healStreamingMarkdownDocument('First line.\n## Title **sub')).toBe(
        'First line.\n## Title **sub**',
      );
    });

    it('does not alter open fenced code blocks', () => {
      const code = '```typescript\nconst a = **broken';
      expect(healStreamingMarkdownDocument(code)).toBe(code);
    });

    it('holds incomplete block prefix at EOF and releases empty trailing lines untouched', () => {
      expect(healStreamingMarkdownDocument('Intro paragraph\n##')).toBe(
        'Intro paragraph\n',
      );
      expect(healStreamingMarkdownDocument('Intro paragraph\n')).toBe(
        'Intro paragraph\n',
      );
    });
  });
});
