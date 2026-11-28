/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { renderWithProviders } from '../../test-utils/render.js';
import { AboutBox } from './AboutBox.js';
import { describe, it, expect, vi } from 'vitest';

// Mock GIT_COMMIT_INFO
vi.mock('../../generated/git-commit.js', () => ({
  GIT_COMMIT_INFO: 'mock-commit-hash',
}));

describe('AboutBox', () => {
  const defaultProps = {
    cliVersion: '1.0.0',
    osVersion: 'macOS',
    sandboxEnv: 'default',
    modelVersion: 'gemini-pro',
    selectedAuthType: 'nvidia-nim',
    ideClient: '',
  };

  it('renders with required props', async () => {
    const { lastFrame, unmount } = await renderWithProviders(
      <AboutBox {...defaultProps} />,
    );
    const output = lastFrame();
    expect(output).toContain('About M31A');
    expect(output).toContain('1.0.0');
    expect(output).toContain('mock-commit-hash');
    expect(output).toContain('gemini-pro');
    expect(output).toContain('default');
    expect(output).toContain('macOS');
    expect(output).toContain('nvidia-nim');
    unmount();
  });

  it.each([
    ['ideClient', 'vscode', 'IDE Client'],
    ['tier', 'Enterprise', 'Tier'],
  ])('renders optional prop %s', async (prop, value, label) => {
    const props = { ...defaultProps, [prop]: value };
    const { lastFrame, unmount } = await renderWithProviders(
      <AboutBox {...props} />,
    );
    const output = lastFrame();
    expect(output).toContain(label);
    expect(output).toContain(value);
    unmount();
  });

  it('renders Provider and Endpoint correctly', async () => {
    const props = { ...defaultProps, selectedAuthType: 'api-key' };
    const { lastFrame, unmount } = await renderWithProviders(
      <AboutBox {...props} />,
    );
    const output = lastFrame();
    expect(output).toContain('Provider');
    expect(output).toContain('api-key');
    expect(output).toContain('Endpoint');
    unmount();
  });
});
