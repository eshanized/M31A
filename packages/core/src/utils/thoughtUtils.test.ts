/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect } from 'vitest';
import {
  allocateThoughtTokenCounts,
  appendThoughtSummary,
  estimateThoughtTokens,
  parseThought,
} from './thoughtUtils.js';

describe('parseThought', () => {
  it.each([
    {
      name: 'a standard thought with subject and description',
      rawText: '**Subject:** This is the description.',
      expected: {
        subject: 'Subject:',
        description: 'This is the description.',
      },
    },
    {
      name: 'leading and trailing whitespace in the raw string',
      rawText: '  **Subject** description with spaces   ',
      expected: { subject: 'Subject', description: 'description with spaces' },
    },
    {
      name: 'whitespace surrounding the subject content',
      rawText: '** Subject  **',
      expected: { subject: 'Subject', description: '' },
    },
    {
      name: 'a thought with only a subject',
      rawText: '**Only Subject**',
      expected: { subject: 'Only Subject', description: '' },
    },
    {
      name: 'a thought with only a description (no subject)',
      rawText: 'This is just a description.',
      expected: { subject: '', description: 'This is just a description.' },
    },
    {
      name: 'an empty string input',
      rawText: '',
      expected: { subject: '', description: '' },
    },
    {
      name: 'newlines within the subject and description',
      rawText:
        '**Multi-line\nSubject**\nHere is a description\nspread across lines.',
      expected: {
        subject: 'Multi-line\nSubject',
        description: 'Here is a description\nspread across lines.',
      },
    },
    {
      name: 'only the first subject if multiple are present',
      rawText: '**First** some text **Second**',
      expected: { subject: 'First', description: 'some text **Second**' },
    },
    {
      name: 'text before and after the subject',
      rawText: 'Prefix text **Subject** Suffix text.',
      expected: {
        subject: 'Subject',
        description: 'Prefix text  Suffix text.',
      },
    },
    {
      name: 'an unclosed subject tag',
      rawText: 'Text with **an unclosed subject',
      expected: { subject: '', description: 'Text with **an unclosed subject' },
    },
    {
      name: 'an empty subject tag',
      rawText: 'A thought with **** in the middle.',
      expected: { subject: '', description: 'A thought with  in the middle.' },
    },
  ])('should correctly parse $name', ({ rawText, expected }) => {
    expect(parseThought(rawText)).toEqual(expected);
  });
});

describe('appendThoughtSummary', () => {
  it('coalesces streaming empty-subject chunks into a single thought', () => {
    let entries = appendThoughtSummary([], { subject: '', description: 'Let' });
    entries = appendThoughtSummary(entries, { subject: '', description: 'me' });
    entries = appendThoughtSummary(entries, {
      subject: '',
      description: 'check.',
    });

    expect(entries).toEqual([{ subject: '', description: 'Let me check.' }]);
  });

  it('starts a new entry when a distinct non-empty subject arrives', () => {
    let entries = appendThoughtSummary([], {
      subject: 'Initial analysis',
      description: 'Checking files',
    });
    entries = appendThoughtSummary(entries, {
      subject: 'Planning',
      description: 'Editing code',
    });

    expect(entries).toEqual([
      { subject: 'Initial analysis', description: 'Checking files' },
      { subject: 'Planning', description: 'Editing code' },
    ]);
  });
});

describe('allocateThoughtTokenCounts', () => {
  it('uses explicit total when provided and distributes proportionally', () => {
    const counts = allocateThoughtTokenCounts(
      [
        { subject: 'A', description: '123456789' },
        { subject: 'B', description: '123456789' },
      ],
      100,
    );
    expect(counts).toEqual([50, 50]);
  });

  it('falls back to text estimation when explicit total is absent or zero', () => {
    const thought = {
      subject: 'Planning',
      description: 'Checking the repository files carefully',
    };
    const estimated = estimateThoughtTokens(thought);
    expect(estimated).toBeGreaterThan(0);
    expect(allocateThoughtTokenCounts([thought], undefined)).toEqual([
      estimated,
    ]);
    expect(allocateThoughtTokenCounts([thought], 0)).toEqual([estimated]);
  });
});
