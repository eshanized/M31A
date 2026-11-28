/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { renderWithProviders } from '../../test-utils/render.js';
import { describe, it, expect, vi } from 'vitest';
import { PrivacyNotice } from './PrivacyNotice.js';
import type { Config } from '@m31a/core';

describe('PrivacyNotice', () => {
  const onExit = vi.fn();
  const mockConfig = {} as Config;

  it('renders M31A Privacy Notice with NVIDIA NIM details', async () => {
    const { lastFrame, unmount } = await renderWithProviders(
      <PrivacyNotice config={mockConfig} onExit={onExit} />,
    );

    const frame = lastFrame();
    expect(frame).toContain('M31A Privacy Notice');
    expect(frame).toContain('NVIDIA NIM');
    expect(frame).toContain('NVIDIA_API_KEY');
    unmount();
  });
});
