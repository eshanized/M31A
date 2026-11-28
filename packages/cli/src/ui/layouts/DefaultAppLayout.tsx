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
import { ExitWarning } from '../components/ExitWarning.js';
import { Sidebar } from '../components/Sidebar.js';
import { useUIState } from '../contexts/UIStateContext.js';
import { useFlickerDetector } from '../hooks/useFlickerDetector.js';
import { useAlternateBuffer } from '../hooks/useAlternateBuffer.js';
import { CopyModeWarning } from '../components/CopyModeWarning.js';
import { BackgroundTaskDisplay } from '../components/BackgroundTaskDisplay.js';
import { StreamingState } from '../types.js';
import { useInputState } from '../contexts/InputContext.js';

export const DefaultAppLayout: React.FC = () => {
  const uiState = useUIState();
  const { copyModeEnabled } = useInputState();
  const isAlternateBuffer = useAlternateBuffer();

  const { rootUiRef, terminalHeight } = uiState;
  useFlickerDetector(rootUiRef, terminalHeight);

  const sidebarWidth = uiState.sidebarWidth ?? 0;
  const hasSidebar = sidebarWidth > 0;
  const contentWidth = uiState.mainAreaWidth || uiState.terminalWidth;

  const showBackgroundTasks =
    uiState.isBackgroundTaskVisible &&
    uiState.backgroundTasks.size > 0 &&
    uiState.activeBackgroundTaskPid &&
    uiState.backgroundTaskHeight > 0 &&
    uiState.streamingState !== StreamingState.WaitingForConfirmation;

  const controlsNode = (
    <Box
      flexDirection="column"
      ref={uiState.mainControlsRef}
      flexShrink={0}
      flexGrow={0}
      width={contentWidth}
      height={copyModeEnabled ? uiState.stableControlsHeight : undefined}
    >
      <Notifications />
      <CopyModeWarning />

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
  );

  const backgroundTaskNode = showBackgroundTasks ? (
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
  ) : null;

  // In inline mode, Ink redraws the whole terminal (erasing the scrollback and
  // printing every earlier <Static> output again) whenever the dynamic UI is
  // as tall as the terminal. Keep it at least one row shorter; if its content
  // is momentarily taller, the top is clipped so the input stays visible. The
  // wrapper is always rendered so toggling the cap never remounts <Static>.
  const capInlineHeight = !isAlternateBuffer && uiState.constrainHeight;
  const mainContentNode = <MainContent />;
  const bottomPaneNode = (
    <>
      {backgroundTaskNode}
      {controlsNode}
    </>
  );

  return (
    <Box
      flexDirection="column"
      flexShrink={0}
      flexGrow={0}
      maxHeight={capInlineHeight ? Math.max(1, terminalHeight - 1) : undefined}
      overflowY={capInlineHeight ? 'hidden' : undefined}
      justifyContent={capInlineHeight ? 'flex-end' : undefined}
    >
      <Box
        flexDirection={isAlternateBuffer ? 'row' : 'column'}
        width={uiState.terminalWidth}
        height={isAlternateBuffer ? terminalHeight : undefined}
        paddingBottom={isAlternateBuffer ? 1 : undefined}
        flexShrink={0}
        flexGrow={0}
        ref={uiState.rootUiRef}
      >
        {isAlternateBuffer ? (
          <>
            <Box
              flexDirection="column"
              width={hasSidebar ? contentWidth : uiState.terminalWidth}
              height="100%"
              flexShrink={0}
              flexGrow={0}
            >
              {mainContentNode}
              {bottomPaneNode}
            </Box>
            {hasSidebar && (
              <>
                <Box width={1} flexShrink={0} />
                <Sidebar width={sidebarWidth} />
              </>
            )}
          </>
        ) : (
          <>
            {mainContentNode}
            {hasSidebar ? (
              <Box
                flexDirection="row"
                width={uiState.terminalWidth}
                alignItems="flex-end"
                flexShrink={0}
              >
                <Box
                  flexDirection="column"
                  width={contentWidth}
                  flexShrink={0}
                  flexGrow={0}
                >
                  {bottomPaneNode}
                </Box>
                <Box width={1} flexShrink={0} />
                <Sidebar width={sidebarWidth} compact />
              </Box>
            ) : (
              bottomPaneNode
            )}
          </>
        )}
      </Box>
    </Box>
  );
};
