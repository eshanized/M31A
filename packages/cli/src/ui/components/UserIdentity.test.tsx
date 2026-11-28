/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { renderWithProviders } from '../../test-utils/render.js';
import { UserIdentity } from './UserIdentity.js';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  makeFakeConfig,
  AuthType,
  type ContentGeneratorConfig,
} from '@m31a/core';

describe('<UserIdentity />', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should render NVIDIA NIM provider label', async () => {
    const mockConfig = makeFakeConfig();
    vi.spyOn(mockConfig, 'getContentGeneratorConfig').mockReturnValue({
      authType: AuthType.USE_NVIDIA_NIM,
      model: 'nvidia/nemotron-3-ultra-550b-a55b',
    } as unknown as ContentGeneratorConfig);
    vi.spyOn(mockConfig, 'getUserTierName').mockReturnValue(undefined);

    const { lastFrame, unmount } = await renderWithProviders(
      <UserIdentity config={mockConfig} />,
    );

    const output = lastFrame();
    expect(output).toContain('Provider: NVIDIA NIM');
    expect(output).not.toContain('/upgrade');
    unmount();
  });

  it('should render provider on the very first frame (regression test)', async () => {
    const mockConfig = makeFakeConfig();
    vi.spyOn(mockConfig, 'getContentGeneratorConfig').mockReturnValue({
      authType: AuthType.USE_NVIDIA_NIM,
      model: 'nvidia/nemotron-3-ultra-550b-a55b',
    } as unknown as ContentGeneratorConfig);
    vi.spyOn(mockConfig, 'getUserTierName').mockReturnValue(undefined);

    const { lastFrameRaw, unmount } = await renderWithProviders(
      <UserIdentity config={mockConfig} />,
    );

    const output = lastFrameRaw();
    expect(output).toContain('NVIDIA NIM');
    unmount();
  });

  it('should render plan name when tier is provided', async () => {
    const mockConfig = makeFakeConfig();
    vi.spyOn(mockConfig, 'getContentGeneratorConfig').mockReturnValue({
      authType: AuthType.USE_NVIDIA_NIM,
      model: 'nvidia/nemotron-3-ultra-550b-a55b',
    } as unknown as ContentGeneratorConfig);
    vi.spyOn(mockConfig, 'getUserTierName').mockReturnValue('Premium Plan');

    const { lastFrame, unmount } = await renderWithProviders(
      <UserIdentity config={mockConfig} />,
    );

    const output = lastFrame();
    expect(output).toContain('Provider: NVIDIA NIM');
    expect(output).toContain('Plan: Premium Plan');
    expect(output).not.toContain('/upgrade');
    unmount();
  });

  it('should not render if authType is missing', async () => {
    const mockConfig = makeFakeConfig();
    vi.spyOn(mockConfig, 'getContentGeneratorConfig').mockReturnValue(
      {} as unknown as ContentGeneratorConfig,
    );

    const { lastFrame, unmount } = await renderWithProviders(
      <UserIdentity config={mockConfig} />,
    );

    expect(lastFrame({ allowEmpty: true })).toBe('');
    unmount();
  });

  it('should render custom authType labels as fallback', async () => {
    const mockConfig = makeFakeConfig();
    vi.spyOn(mockConfig, 'getContentGeneratorConfig').mockReturnValue({
      authType: 'custom-endpoint',
      model: 'nvidia/nemotron-3-ultra-550b-a55b',
    } as unknown as ContentGeneratorConfig);
    vi.spyOn(mockConfig, 'getUserTierName').mockReturnValue(undefined);

    const { lastFrame, unmount } = await renderWithProviders(
      <UserIdentity config={mockConfig} />,
    );

    const output = lastFrame();
    expect(output).toContain('Provider: custom-endpoint');
    unmount();
  });
});
