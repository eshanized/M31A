/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { render } from '../../test-utils/render.js';
import { M31aRespondingSpinner } from './M31aRespondingSpinner.js';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useStreamingContext } from '../contexts/StreamingContext.js';
import { Text, useIsScreenReaderEnabled } from 'ink';
import { StreamingState } from '../types.js';
import {
  SCREEN_READER_LOADING,
  SCREEN_READER_RESPONDING,
} from '../textConstants.js';

vi.mock('../contexts/StreamingContext.js');
vi.mock('ink', async (importOriginal) => {
  const actual = await importOriginal<typeof import('ink')>();
  return {
    ...actual,
    useIsScreenReaderEnabled: vi.fn(),
  };
});

vi.mock('./M31aSpinner.js', () => ({
  M31aSpinner: ({ altText }: { altText?: string }) => (
    <Text>M31aSpinner {altText}</Text>
  ),
}));

describe('M31aRespondingSpinner', () => {
  const mockUseStreamingContext = vi.mocked(useStreamingContext);
  const mockUseIsScreenReaderEnabled = vi.mocked(useIsScreenReaderEnabled);

  beforeEach(() => {
    vi.clearAllMocks();
    mockUseIsScreenReaderEnabled.mockReturnValue(false);
  });

  it('renders spinner when responding', async () => {
    mockUseStreamingContext.mockReturnValue(StreamingState.Responding);
    const { lastFrame, unmount } = await render(<M31aRespondingSpinner />);
    expect(lastFrame()).toContain('M31aSpinner');
    unmount();
  });

  it('renders screen reader text when responding and screen reader enabled', async () => {
    mockUseStreamingContext.mockReturnValue(StreamingState.Responding);
    mockUseIsScreenReaderEnabled.mockReturnValue(true);
    const { lastFrame, unmount } = await render(<M31aRespondingSpinner />);
    expect(lastFrame()).toContain(SCREEN_READER_RESPONDING);
    unmount();
  });

  it('renders nothing when not responding and no non-responding display', async () => {
    mockUseStreamingContext.mockReturnValue(StreamingState.Idle);
    const { lastFrame, unmount } = await render(<M31aRespondingSpinner />);
    expect(lastFrame({ allowEmpty: true })).toBe('');
    unmount();
  });

  it('renders non-responding display when provided', async () => {
    mockUseStreamingContext.mockReturnValue(StreamingState.Idle);
    const { lastFrame, unmount } = await render(
      <M31aRespondingSpinner nonRespondingDisplay="Waiting..." />,
    );
    expect(lastFrame()).toContain('Waiting...');
    unmount();
  });

  it('renders screen reader loading text when non-responding display provided and screen reader enabled', async () => {
    mockUseStreamingContext.mockReturnValue(StreamingState.Idle);
    mockUseIsScreenReaderEnabled.mockReturnValue(true);
    const { lastFrame, unmount } = await render(
      <M31aRespondingSpinner nonRespondingDisplay="Waiting..." />,
    );
    expect(lastFrame()).toContain(SCREEN_READER_LOADING);
    unmount();
  });
});
