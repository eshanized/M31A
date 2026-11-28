/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, vi } from 'vitest';
import { renderWithProviders } from '../../test-utils/render.js';
import { createMockSettings } from '../../test-utils/settings.js';
import { VoiceModelDialog } from './VoiceModelDialog.js';
import { act } from 'react';
import { waitFor } from '../../test-utils/async.js';
import { SettingScope } from '../../config/settings.js';

vi.mock('@m31a/core', async () => {
  const actual = await vi.importActual('@m31a/core');
  return {
    ...actual,
    isBinaryAvailable: vi.fn().mockReturnValue(true),
    WhisperModelManager: vi.fn().mockImplementation(() => ({
      isModelInstalled: vi.fn().mockReturnValue(false),
      on: vi.fn(),
      off: vi.fn(),
      downloadModel: vi.fn(),
    })),
  };
});

describe('VoiceModelDialog', () => {
  it('should render local Whisper model selection options without Google Cloud warnings', async () => {
    const onClose = vi.fn();
    const { lastFrame, waitUntilReady } = await renderWithProviders(
      <VoiceModelDialog onClose={onClose} />,
    );

    await waitUntilReady();

    const frame = lastFrame();
    expect(frame).toContain('Select Whisper Model');
    expect(frame).toContain('Tiny (EN)');
    expect(frame).not.toContain('Gemini Live');
  });

  it('should update settings and close dialog when an installed whisper model is selected', async () => {
    const onClose = vi.fn();
    const settings = createMockSettings();
    const setValueSpy = vi.spyOn(settings, 'setValue');

    const core = await import('@m31a/core');
    vi.mocked(core.WhisperModelManager).mockImplementation(
      () =>
        ({
          isModelInstalled: vi.fn().mockReturnValue(true),
          on: vi.fn(),
          off: vi.fn(),
          downloadModel: vi.fn(),
        }) as unknown as InstanceType<typeof core.WhisperModelManager>,
    );

    const { waitUntilReady, stdin } = await renderWithProviders(
      <VoiceModelDialog onClose={onClose} />,
      { settings },
    );

    await waitUntilReady();

    await act(async () => {
      stdin.write('\r');
    });

    await waitFor(() => {
      expect(setValueSpy).toHaveBeenCalledWith(
        SettingScope.User,
        'experimental.voice.backend',
        'whisper',
      );
      expect(setValueSpy).toHaveBeenCalledWith(
        SettingScope.User,
        'experimental.voice.whisperModel',
        'ggml-base.en.bin',
      );
      expect(onClose).toHaveBeenCalled();
    });
  });
});
