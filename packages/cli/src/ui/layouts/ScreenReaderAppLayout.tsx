/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { Box } from 'ink';
import { Notifications } from '../components/Notifications.js';
import { MainContent } from '../components/MainContent.js';
import { DialogManager } from '../components/DialogManager.js';
import { Composer } from '../components/Composer.js';
import { Footer } from '../components/Footer.js';
import { ExitWarning } from '../components/ExitWarning.js';
import { CopyModeWarning } from '../components/CopyModeWarning.js';
import { BackgroundTaskDisplay } from '../components/BackgroundTaskDisplay.js';
import { useUIState } from '../contexts/UIStateContext.js';
import { useInputState } from '../contexts/InputContext.js';
import { useFlickerDetector } from '../hooks/useFlickerDetector.js';
import { StreamingState } from '../types.js';
import {
  SCREEN_READER_WIDTH_PERCENT,
  getScreenReaderContentWidth,
} from '../utils/isNarrowWidth.js';

export const ScreenReaderAppLayout: React.FC = () => {
  const uiState = useUIState();
  const { copyModeEnabled } = useInputState();
  const { rootUiRef, terminalHeight } = uiState;
  useFlickerDetector(rootUiRef, terminalHeight);

  const baseWidth = uiState.mainAreaWidth || uiState.terminalWidth;
  const contentWidth = getScreenReaderContentWidth(baseWidth);

  const showBackgroundTasks =
    uiState.isBackgroundTaskVisible &&
    uiState.backgroundTasks.size > 0 &&
    uiState.activeBackgroundTaskPid &&
    uiState.backgroundTaskHeight > 0 &&
    uiState.streamingState !== StreamingState.WaitingForConfirmation;

  return (
    <Box
      flexDirection="column"
      width={SCREEN_READER_WIDTH_PERCENT}
      height="100%"
      ref={uiState.rootUiRef}
    >
      <Notifications />
      <Footer />
      <Box flexGrow={1} overflow="hidden">
        <MainContent />
      </Box>
      {showBackgroundTasks && (
        <Box height={uiState.backgroundTaskHeight} flexShrink={0}>
          <BackgroundTaskDisplay
            shells={uiState.backgroundTasks}
            activePid={uiState.activeBackgroundTaskPid!}
            width={contentWidth}
            height={uiState.backgroundTaskHeight}
            isFocused={uiState.embeddedShellFocused && !uiState.dialogsVisible}
            isListOpenProp={uiState.isBackgroundTaskListOpen}
          />
        </Box>
      )}
      <Box
        flexDirection="column"
        ref={uiState.mainControlsRef}
        flexShrink={0}
        flexGrow={0}
        height={copyModeEnabled ? uiState.stableControlsHeight : undefined}
      >
        {copyModeEnabled && <CopyModeWarning />}
        {uiState.customDialog ? (
          uiState.customDialog
        ) : uiState.dialogsVisible ? (
          <DialogManager
            terminalWidth={contentWidth}
            addItem={uiState.historyManager.addItem}
          />
        ) : (
          <Composer isFocused={true} />
        )}
        <ExitWarning />
      </Box>
    </Box>
  );
};
