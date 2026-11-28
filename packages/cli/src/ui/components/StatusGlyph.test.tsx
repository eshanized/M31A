/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { Text, useIsScreenReaderEnabled } from 'ink';
import { renderWithProviders } from '../../test-utils/render.js';
import { StatusGlyph } from './StatusGlyph.js';

vi.mock('ink', async (importOriginal) => {
  const actual = await importOriginal<typeof import('ink')>();
  return {
    ...actual,
    useIsScreenReaderEnabled: vi.fn(() => false),
  };
});

vi.mock('./CliSpinner.js', () => ({
  TMUX_SPINNER_INTERVAL_MS: 120,
  CliSpinner: ({ reducedMotion }: { reducedMotion?: boolean }) => (
    <Text>CliSpinner(reduced={String(Boolean(reducedMotion))})</Text>
  ),
}));

describe('<StatusGlyph />', () => {
  const mockScreenReader = vi.mocked(useIsScreenReaderEnabled);

  beforeEach(() => {
    vi.clearAllMocks();
    mockScreenReader.mockReturnValue(false);
    vi.unstubAllEnvs();
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('renders altText when screen reader is enabled', async () => {
    mockScreenReader.mockReturnValue(true);
    const { lastFrame, unmount } = await renderWithProviders(
      <StatusGlyph active={true} altText="responding" />,
    );
    expect(lastFrame()).toContain('responding');
    unmount();
  });

  it('renders nonRespondingDisplay when inactive and screen reader is off', async () => {
    const { lastFrame, unmount } = await renderWithProviders(
      <StatusGlyph active={false} nonRespondingDisplay="⠏" />,
    );
    expect(lastFrame()).toContain('⠏');
    unmount();
  });

  it('renders nonRespondingAltText when inactive and screen reader is enabled', async () => {
    mockScreenReader.mockReturnValue(true);
    const { lastFrame, unmount } = await renderWithProviders(
      <StatusGlyph
        active={false}
        nonRespondingDisplay="⠏"
        nonRespondingAltText="loading"
      />,
    );
    expect(lastFrame()).toContain('loading');
    expect(lastFrame()).not.toContain('⠏');
    unmount();
  });

  it('renders null when inactive and no nonRespondingDisplay is set', async () => {
    const { lastFrame, unmount } = await renderWithProviders(
      <StatusGlyph active={false} />,
    );
    expect(lastFrame({ allowEmpty: true })).toBe('');
    unmount();
  });

  it('honors M31A_REDUCED_MOTION=1 on gradient variant', async () => {
    vi.stubEnv('M31A_REDUCED_MOTION', '1');
    const { lastFrame, unmount } = await renderWithProviders(
      <StatusGlyph active={true} variant="gradient" />,
    );
    expect(lastFrame()).toContain('CliSpinner(reduced=true)');
    unmount();
  });
});
