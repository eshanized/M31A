/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { act } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '../../test-utils/render.js';
import { waitFor } from '../../test-utils/async.js';
import {
  CommandPaletteDialog,
  buildCommandPaletteActions,
  type CommandPaletteAction,
} from './CommandPaletteDialog.js';
import { CommandKind, type SlashCommand } from '../commands/types.js';

describe('CommandPaletteDialog', () => {
  const sampleActions: CommandPaletteAction[] = [
    {
      id: 'quick:theme',
      label: 'Switch Theme',
      description: 'Choose UI color theme',
      category: 'Quick Actions',
      shortcut: 'Alt+X t',
      suggested: true,
      onSelect: vi.fn(),
    },
    {
      id: 'slash:clear',
      label: '/clear',
      description: 'Clear screen and conversation history',
      category: 'Commands',
      onSelect: vi.fn(),
    },
    {
      id: 'slash:model',
      label: '/model',
      description: 'Select active NVIDIA NIM model',
      category: 'Commands',
      onSelect: vi.fn(),
    },
  ];

  it('shows Suggested section when query is empty and executes selected item on Enter', async () => {
    const onSelectTheme = vi.fn();
    const onSelectModel = vi.fn();
    const onClose = vi.fn();
    const actions: CommandPaletteAction[] = [
      { ...sampleActions[0], onSelect: onSelectTheme },
      { ...sampleActions[1] },
      { ...sampleActions[2], onSelect: onSelectModel },
    ];

    const { lastFrame, stdin, waitUntilReady, unmount } =
      await renderWithProviders(
        <CommandPaletteDialog actions={actions} onClose={onClose} />,
      );
    await waitUntilReady();

    expect(lastFrame()).toContain('Command Palette');
    expect(lastFrame()).toContain('Suggested');
    expect(lastFrame()).toContain('Switch Theme');

    await act(async () => {
      stdin.write('mod');
    });
    await waitFor(() => {
      expect(lastFrame()).toContain('/model');
      expect(lastFrame()).not.toContain('Switch Theme');
    });

    await act(async () => {
      stdin.write('\r');
    });
    await waitFor(() => {
      expect(onClose).toHaveBeenCalledTimes(1);
      expect(onSelectModel).toHaveBeenCalledTimes(1);
    });
    expect(onSelectTheme).not.toHaveBeenCalled();
    unmount();
  });

  it('closes on Escape without invoking action', async () => {
    const onSelect = vi.fn();
    const onClose = vi.fn();
    const { stdin, waitUntilReady, unmount } = await renderWithProviders(
      <CommandPaletteDialog
        actions={[{ ...sampleActions[0], onSelect }]}
        onClose={onClose}
      />,
    );
    await waitUntilReady();

    await act(async () => {
      stdin.write('\u001b');
    });
    await waitFor(() => {
      expect(onClose).toHaveBeenCalledTimes(1);
    });
    expect(onSelect).not.toHaveBeenCalled();
    unmount();
  });

  it('builds deduplicated palette actions from slash commands and subcommands', () => {
    const slashCommands: SlashCommand[] = [
      {
        name: 'stash',
        description: 'Manage stashed prompts',
        kind: CommandKind.BUILT_IN,
        subCommands: [
          {
            name: 'list',
            description: 'List stashed prompts',
            kind: CommandKind.BUILT_IN,
          },
          {
            name: 'hidden-sub',
            description: 'Hidden',
            kind: CommandKind.BUILT_IN,
            hidden: true,
          },
        ],
      },
      {
        name: 'secret',
        description: 'Hidden command',
        kind: CommandKind.BUILT_IN,
        hidden: true,
      },
    ];

    const onRun = vi.fn();
    const actions = buildCommandPaletteActions(slashCommands, onRun, [
      {
        id: 'slash:stash',
        label: '/stash',
        description: 'Override',
        onSelect: vi.fn(),
      },
    ]);

    expect(actions.map((a) => a.label)).toEqual(['/stash', '/stash list']);
    actions[1]?.onSelect();
    expect(onRun).toHaveBeenCalledWith('stash list');
  });
});
