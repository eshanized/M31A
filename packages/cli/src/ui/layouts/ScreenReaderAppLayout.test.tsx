/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { Text } from 'ink';
import { render } from '../../test-utils/render.js';
import { ScreenReaderAppLayout } from './ScreenReaderAppLayout.js';
import { useInputState } from '../contexts/InputContext.js';
import { StreamingState } from '../types.js';
import type { UIState } from '../contexts/UIStateContext.js';
import type { BackgroundTask } from '../hooks/useExecutionLifecycle.js';

vi.mock('../contexts/InputContext.js');
vi.mock('../hooks/useFlickerDetector.js', () => ({
  useFlickerDetector: vi.fn(),
}));

const mockUIState = {
  rootUiRef: { current: null },
  terminalHeight: 24,
  terminalWidth: 100,
  mainAreaWidth: 100,
  sidebarWidth: 0,
  backgroundTasks: new Map<number, BackgroundTask>(),
  activeBackgroundTaskPid: null as number | null,
  backgroundTaskHeight: 6,
  embeddedShellFocused: false,
  dialogsVisible: false,
  streamingState: StreamingState.Idle,
  isBackgroundTaskListOpen: false,
  mainControlsRef: vi.fn(),
  customDialog: null,
  historyManager: { addItem: vi.fn() },
  history: [],
  pendingHistoryItems: [],
  slashCommands: [],
  constrainHeight: false,
  availableTerminalHeight: 20,
  stableControlsHeight: 4,
  activePtyId: null,
  isBackgroundTaskVisible: true,
} as unknown as UIState;

vi.mock('../contexts/UIStateContext.js', () => ({
  useUIState: () => mockUIState,
}));

const dialogManagerMock = vi.fn(
  ({ terminalWidth }: { terminalWidth: number }) => (
    <Text>DialogManager(w={terminalWidth})</Text>
  ),
);

vi.mock('../components/Notifications.js', () => ({
  Notifications: () => <Text>Notifications</Text>,
}));
vi.mock('../components/Footer.js', () => ({
  Footer: () => <Text>Footer</Text>,
}));
vi.mock('../components/MainContent.js', () => ({
  MainContent: () => <Text>MainContent</Text>,
}));
vi.mock('../components/DialogManager.js', () => ({
  DialogManager: (props: { terminalWidth: number }) => dialogManagerMock(props),
}));
vi.mock('../components/Composer.js', () => ({
  Composer: ({ isFocused }: { isFocused?: boolean }) => (
    <Text>Composer(focused={String(isFocused)})</Text>
  ),
}));
vi.mock('../components/ExitWarning.js', () => ({
  ExitWarning: () => <Text>ExitWarning</Text>,
}));
vi.mock('../components/CopyModeWarning.js', () => ({
  CopyModeWarning: () => <Text>CopyModeWarning</Text>,
}));
vi.mock('../components/BackgroundTaskDisplay.js', () => ({
  BackgroundTaskDisplay: ({ width }: { width: number }) => (
    <Text>BackgroundTaskDisplay(w={width})</Text>
  ),
}));

describe('<ScreenReaderAppLayout />', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(useInputState).mockReturnValue({
      copyModeEnabled: false,
    } as unknown as ReturnType<typeof useInputState>);
    mockUIState.backgroundTasks = new Map();
    mockUIState.activeBackgroundTaskPid = null;
    mockUIState.streamingState = StreamingState.Idle;
    mockUIState.dialogsVisible = false;
    mockUIState.customDialog = null;
    mockUIState.terminalWidth = 100;
    mockUIState.mainAreaWidth = 100;
  });

  it('renders Notifications, Footer, MainContent, focused Composer, and ExitWarning in order', async () => {
    const { lastFrame, unmount } = await render(<ScreenReaderAppLayout />);
    const frame = lastFrame();

    const notificationsIdx = frame.indexOf('Notifications');
    const footerIdx = frame.indexOf('Footer');
    const mainIdx = frame.indexOf('MainContent');
    const composerIdx = frame.indexOf('Composer(focused=true)');
    const exitIdx = frame.indexOf('ExitWarning');

    expect(notificationsIdx).toBeGreaterThanOrEqual(0);
    expect(footerIdx).toBeGreaterThan(notificationsIdx);
    expect(mainIdx).toBeGreaterThan(footerIdx);
    expect(composerIdx).toBeGreaterThan(mainIdx);
    expect(exitIdx).toBeGreaterThan(composerIdx);
    expect(frame).not.toContain('CopyModeWarning');
    unmount();
  });

  it('passes 90% content width to DialogManager when dialogsVisible is true', async () => {
    mockUIState.dialogsVisible = true;
    const { lastFrame, unmount } = await render(<ScreenReaderAppLayout />);
    const frame = lastFrame();
    expect(frame).toContain('DialogManager(w=90)');
    expect(frame).not.toContain('Composer');
    unmount();
  });

  it('renders customDialog instead of DialogManager or Composer when provided', async () => {
    mockUIState.customDialog = <Text>CustomDialogNode</Text>;
    const { lastFrame, unmount } = await render(<ScreenReaderAppLayout />);
    const frame = lastFrame();
    expect(frame).toContain('CustomDialogNode');
    expect(frame).not.toContain('Composer');
    unmount();
  });

  it('renders CopyModeWarning and BackgroundTaskDisplay when enabled', async () => {
    vi.mocked(useInputState).mockReturnValue({
      copyModeEnabled: true,
    } as unknown as ReturnType<typeof useInputState>);
    mockUIState.backgroundTasks.set(42, {
      pid: 42,
      command: 'echo hi',
      output: 'hi',
      isBinary: false,
      binaryBytesReceived: 0,
      status: 'running',
    });
    mockUIState.activeBackgroundTaskPid = 42;

    const { lastFrame, unmount } = await render(<ScreenReaderAppLayout />);
    const frame = lastFrame();
    expect(frame).toContain('BackgroundTaskDisplay(w=90)');
    expect(frame).toContain('CopyModeWarning');
    unmount();
  });
});
