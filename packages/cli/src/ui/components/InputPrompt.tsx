/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import {
  useCallback,
  useEffect,
  useState,
  useRef,
  useMemo,
  Fragment,
} from 'react';
import { Box, Text, useStdout, type DOMElement } from 'ink';
import { SuggestionsDisplay } from './SuggestionsDisplay.js';
import { theme } from '../semantic-colors.js';
import { useInputHistory } from '../hooks/useInputHistory.js';
import {
  ScrollableList,
  type ScrollableListRef,
} from './shared/ScrollableList.js';
import { ListeningIndicator } from './ListeningIndicator.js';
import { HalfLinePaddedBox } from './shared/HalfLinePaddedBox.js';
import { expandPastePlaceholders } from './shared/text-buffer.js';
import { cpSlice } from '../utils/textUtils.js';
import chalk from 'chalk';
import { useShellHistory } from '../hooks/useShellHistory.js';
import { useReverseSearchCompletion } from '../hooks/useReverseSearchCompletion.js';
import {
  useCommandCompletion,
  CompletionMode,
} from '../hooks/useCommandCompletion.js';
import { coreEvents, type ApprovalMode, type Config } from '@m31a/core';
import type { CommandContext, SlashCommand } from '../commands/types.js';
import type { Key } from '../hooks/useKeypress.js';
import type { VimMode } from '../contexts/VimModeContext.js';
import { useVoiceMode } from '../hooks/useVoiceMode.js';
import { useKittyKeyboardProtocol } from '../hooks/useKittyKeyboardProtocol.js';
import { isSlashCommand } from '../utils/commandUtils.js';
import { parseSlashCommand } from '../../utils/commands.js';
import {
  SCREEN_READER_REVERSE_SEARCH_PREFIX,
  SCREEN_READER_USER_PREFIX,
} from '../textConstants.js';
import { useUIState } from '../contexts/UIStateContext.js';
import { useInputState } from '../contexts/InputContext.js';
import { appEvents, AppEvent } from '../../utils/events.js';
import { useSettings } from '../contexts/SettingsContext.js';
import { StreamingState } from '../types.js';
import { useUIActions } from '../contexts/UIActionsContext.js';
import { useAlternateBuffer } from '../hooks/useAlternateBuffer.js';
import { useIsHelpDismissKey } from '../utils/shortcutsHelp.js';
import { useRepeatedKeyPress } from '../hooks/useRepeatedKeyPress.js';
import { useKeyMatchers } from '../hooks/useKeyMatchers.js';
import {
  SCROLLBAR_GUTTER_WIDTH,
  DOUBLE_TAB_CLEAN_UI_TOGGLE_WINDOW_MS,
  isTerminalPasteTrusted,
  type ScrollableItem,
  calculatePromptWidths,
  isLargePaste,
  tryTogglePasteExpansion,
  computeGhostTextLines,
  getInputPromptStatusPresentation,
} from './input/inputPromptUtils.js';
import { useClipboardPaste } from './input/useClipboardPaste.js';
import { useInputPromptMouse } from './input/useInputPromptMouse.js';
import { useInputPromptKeybindings } from './input/useInputPromptKeybindings.js';
import { InputPromptLine } from './input/InputPromptLine.js';

export interface InputPromptProps {
  onSubmit: (value: string) => void;
  onClearScreen: () => void;
  config: Config;
  slashCommands: readonly SlashCommand[];
  commandContext: CommandContext;
  placeholder?: string;
  focus?: boolean;
  setShellModeActive: (value: boolean) => void;
  approvalMode: ApprovalMode;
  onEscapePromptChange?: (showPrompt: boolean) => void;
  onSuggestionsVisibilityChange?: (visible: boolean) => void;
  vimHandleInput?: (key: Key) => boolean;
  vimEnabled?: boolean;
  vimMode?: VimMode;
  isEmbeddedShellFocused?: boolean;
  setQueueErrorMessage: (message: string | null) => void;
  streamingState: StreamingState;
  popAllMessages?: () => string | undefined;
  onQueueMessage?: (message: string) => void;
  suggestionsPosition?: 'above' | 'below';
  setBannerVisible: (visible: boolean) => void;
}

export {
  isTerminalPasteTrusted,
  type ScrollableItem,
  calculatePromptWidths,
  isLargePaste,
  tryTogglePasteExpansion,
};

export const InputPrompt: React.FC<InputPromptProps> = ({
  onSubmit,
  onClearScreen,
  config,
  slashCommands,
  commandContext,
  placeholder = '  Type your message or @path/to/file',
  focus = true,
  setShellModeActive,
  approvalMode,
  onEscapePromptChange,
  onSuggestionsVisibilityChange,
  vimHandleInput,
  vimEnabled,
  vimMode,
  isEmbeddedShellFocused,
  setQueueErrorMessage,
  streamingState,
  popAllMessages,
  onQueueMessage,
  suggestionsPosition = 'below',
  setBannerVisible,
}) => {
  const inputState = useInputState();
  const {
    buffer,
    userMessages,
    shellModeActive,
    copyModeEnabled,
    inputWidth,
    suggestionsWidth,
  } = inputState;
  const isHelpDismissKey = useIsHelpDismissKey();
  const keyMatchers = useKeyMatchers();
  const { stdout } = useStdout();
  const { merged: settings } = useSettings();
  const kittyProtocol = useKittyKeyboardProtocol();
  const {
    setEmbeddedShellFocused,
    setShortcutsHelpVisible,
    toggleCleanUiDetailsVisible,
    setVoiceModeEnabled,
  } = useUIActions();
  const {
    isFocused: isShellFocused,
    terminalWidth,
    mainAreaWidth,
    activePtyId,
    history,
    backgroundTasks,
    backgroundTaskHeight,
    shortcutsHelpVisible,
    isVoiceModeEnabled,
  } = useUIState();
  const promptContainerWidth = mainAreaWidth || terminalWidth;
  const [suppressCompletion, setSuppressCompletion] = useState(false);
  const { handlePress: registerPlainTabPress, resetCount: resetPlainTabPress } =
    useRepeatedKeyPress({
      windowMs: DOUBLE_TAB_CLEAN_UI_TOGGLE_WINDOW_MS,
    });
  const [showEscapePrompt, setShowEscapePrompt] = useState(false);
  const { handlePress: handleEscPress, resetCount: resetEscapeState } =
    useRepeatedKeyPress({
      windowMs: 500,
      onRepeat: (count) => {
        if (count === 1) {
          setShowEscapePrompt(true);
        } else if (count === 2) {
          resetEscapeState();
          if (buffer.text.length > 0) {
            buffer.setText('');
            resetTurnBaseline();
            resetCompletionState();
          } else if (history.length > 0) {
            onSubmit('/rewind');
          } else {
            coreEvents.emitFeedback('info', 'Nothing to rewind to');
          }
        }
      },
      onReset: () => setShowEscapePrompt(false),
    });
  const [recentUnsafePasteTime, setRecentUnsafePasteTime] = useState<
    number | null
  >(null);
  const pasteTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const innerBoxRef = useRef<DOMElement>(null);
  const hasUserNavigatedSuggestions = useRef(false);
  const listRef = useRef<ScrollableListRef<ScrollableItem>>(null);

  const { isRecording, handleVoiceInput, resetTurnBaseline } = useVoiceMode({
    buffer,
    settings,
    setQueueErrorMessage,
    isVoiceModeEnabled,
    setVoiceModeEnabled,
    keyMatchers,
  });

  const [reverseSearchActive, setReverseSearchActive] = useState(false);
  const [commandSearchActive, setCommandSearchActive] = useState(false);
  const [textBeforeReverseSearch, setTextBeforeReverseSearch] = useState('');
  const [cursorPosition, setCursorPosition] = useState<[number, number]>([
    0, 0,
  ]);
  const [expandedSuggestionIndex, setExpandedSuggestionIndex] =
    useState<number>(-1);
  const shellHistory = useShellHistory(config.getProjectRoot(), config.storage);
  const shellHistoryData = shellHistory.history;

  const completion = useCommandCompletion({
    buffer,
    cwd: config.getTargetDir(),
    slashCommands,
    commandContext,
    reverseSearchActive,
    shellModeActive,
    config,
    active: !suppressCompletion,
  });

  const reverseSearchCompletion = useReverseSearchCompletion(
    buffer,
    shellHistoryData,
    reverseSearchActive,
  );

  const reversedUserMessages = useMemo(
    () => [...userMessages].reverse(),
    [userMessages],
  );

  const commandSearchCompletion = useReverseSearchCompletion(
    buffer,
    reversedUserMessages,
    commandSearchActive,
  );

  const resetCompletionState = completion.resetCompletionState;
  const resetReverseSearchCompletionState =
    reverseSearchCompletion.resetCompletionState;
  const resetCommandSearchCompletionState =
    commandSearchCompletion.resetCompletionState;

  const activeCompletion = commandSearchActive
    ? commandSearchCompletion
    : reverseSearchActive
      ? reverseSearchCompletion
      : completion;
  const shouldShowSuggestions = activeCompletion.showSuggestions;

  const {
    forceShowShellSuggestions,
    setForceShowShellSuggestions,
    isShellSuggestionsVisible,
  } = completion;

  const effectivePlaceholder = useMemo(() => {
    if (!isVoiceModeEnabled) return placeholder;
    const voiceAction =
      (settings.experimental.voice?.activationMode ?? 'push-to-talk') ===
      'push-to-talk'
        ? 'hold space to talk'
        : 'space to talk';
    return `  Type your message or ${voiceAction} (Esc to exit)`;
  }, [
    isVoiceModeEnabled,
    placeholder,
    settings.experimental.voice?.activationMode,
  ]);

  const showCursor =
    focus && isShellFocused && !isEmbeddedShellFocused && !copyModeEnabled;

  useEffect(() => {
    appEvents.emit(AppEvent.ScrollToBottom);
  }, [buffer.text, buffer.cursor]);

  useEffect(() => {
    if (onEscapePromptChange) {
      onEscapePromptChange(showEscapePrompt);
    }
  }, [showEscapePrompt, onEscapePromptChange]);

  useEffect(
    () => () => {
      if (pasteTimeoutRef.current) {
        clearTimeout(pasteTimeoutRef.current);
      }
    },
    [],
  );

  const handleSubmitAndClear = useCallback(
    (submittedValue: string) => {
      let processedValue = submittedValue;
      if (buffer.pastedContent) {
        processedValue = expandPastePlaceholders(
          processedValue,
          buffer.pastedContent,
        );
      }

      if (shellModeActive) {
        shellHistory.addCommandToHistory(processedValue);
      }
      // Clear the buffer *before* calling onSubmit to prevent potential re-submission
      // if onSubmit triggers a re-render while the buffer still holds the old value.
      buffer.setText('');
      resetTurnBaseline();
      onSubmit(processedValue);
      resetCompletionState();
      resetReverseSearchCompletionState();
    },
    [
      buffer,
      onSubmit,
      resetCompletionState,
      shellModeActive,
      shellHistory,
      resetReverseSearchCompletionState,
      resetTurnBaseline,
    ],
  );

  const customSetTextAndResetCompletionSignal = useCallback(
    (newText: string, cursorPos?: 'start' | 'end' | number) => {
      buffer.setText(newText, cursorPos);
      setSuppressCompletion(true);
    },
    [buffer, setSuppressCompletion],
  );

  const inputHistory = useInputHistory({
    userMessages,
    onSubmit: handleSubmitAndClear,
    isActive:
      (!(completion.showSuggestions && isShellSuggestionsVisible) ||
        completion.suggestions.length === 1) &&
      !shellModeActive,
    currentQuery: buffer.text,
    currentCursorOffset: buffer.getOffset(),
    onChange: customSetTextAndResetCompletionSignal,
  });

  const handleSubmit = useCallback(
    (submittedValue: string) => {
      const trimmedMessage = submittedValue.trim();
      const isSlash = isSlashCommand(trimmedMessage);

      const isShell = shellModeActive;
      if (
        (isSlash || isShell) &&
        streamingState === StreamingState.Responding
      ) {
        if (isSlash) {
          const { commandToExecute } = parseSlashCommand(
            trimmedMessage,
            slashCommands,
          );
          if (commandToExecute?.isSafeConcurrent) {
            handleSubmitAndClear(trimmedMessage);
            return;
          }
        }

        setQueueErrorMessage(
          `${isShell ? 'Shell' : 'Slash'} commands cannot be queued`,
        );
        return;
      }
      inputHistory.handleSubmit(trimmedMessage);
    },
    [
      inputHistory,
      shellModeActive,
      streamingState,
      setQueueErrorMessage,
      slashCommands,
      handleSubmitAndClear,
    ],
  );

  useEffect(() => {
    if (suppressCompletion) {
      resetCompletionState();
      resetReverseSearchCompletionState();
      resetCommandSearchCompletionState();
      setExpandedSuggestionIndex(-1);
    }
  }, [
    suppressCompletion,
    buffer.text,
    resetCompletionState,
    setSuppressCompletion,
    resetReverseSearchCompletionState,
    resetCommandSearchCompletionState,
    setExpandedSuggestionIndex,
  ]);

  const tryLoadQueuedMessages = useCallback(() => {
    if (buffer.text.trim() === '' && popAllMessages) {
      const allMessages = popAllMessages();
      if (allMessages) {
        buffer.setText(allMessages);
        return true;
      } else {
        inputHistory.navigateUp();
      }
      return true;
    }
    return false;
  }, [buffer, popAllMessages, inputHistory]);

  const handleClipboardPaste = useClipboardPaste({
    buffer,
    config,
    stdout,
    settings,
    shortcutsHelpVisible,
    setShortcutsHelpVisible,
  });

  const isAlternateBuffer = useAlternateBuffer();

  useInputPromptMouse({
    innerBoxRef,
    listRef,
    buffer,
    focus,
    copyModeEnabled,
    isEmbeddedShellFocused,
    isAlternateBuffer,
    setSuppressCompletion,
    setEmbeddedShellFocused,
    handleClipboardPaste,
  });

  useInputPromptKeybindings({
    focus,
    isEmbeddedShellFocused,
    copyModeEnabled,
    buffer,
    completion,
    shellModeActive,
    setShellModeActive,
    onClearScreen,
    inputHistory,
    handleSubmit,
    shellHistory,
    reverseSearchCompletion,
    commandSearchCompletion,
    handleClipboardPaste,
    resetCompletionState,
    resetEscapeState,
    vimHandleInput,
    vimEnabled,
    vimMode,
    reverseSearchActive,
    setReverseSearchActive,
    commandSearchActive,
    setCommandSearchActive,
    textBeforeReverseSearch,
    setTextBeforeReverseSearch,
    cursorPosition,
    setCursorPosition,
    recentUnsafePasteTime,
    setRecentUnsafePasteTime,
    pasteTimeoutRef,
    kittyProtocolEnabled: kittyProtocol.enabled,
    shortcutsHelpVisible,
    setShortcutsHelpVisible,
    tryLoadQueuedMessages,
    onQueueMessage,
    setQueueErrorMessage,
    resetReverseSearchCompletionState,
    setBannerVisible,
    activePtyId,
    setEmbeddedShellFocused,
    backgroundTaskCount: backgroundTasks.size,
    backgroundTaskHeight,
    streamingState,
    handleEscPress,
    resetTurnBaseline,
    registerPlainTabPress,
    resetPlainTabPress,
    toggleCleanUiDetailsVisible,
    shouldShowSuggestions,
    isShellSuggestionsVisible,
    forceShowShellSuggestions,
    setForceShowShellSuggestions,
    setSuppressCompletion,
    setExpandedSuggestionIndex,
    hasUserNavigatedSuggestions,
    keyMatchers,
    isHelpDismissKey,
    settings,
    handleVoiceInput,
  });

  const [cursorVisualRowAbsolute, cursorVisualColAbsolute] =
    buffer.visualCursor;

  const { inlineGhost, additionalLines } = useMemo(
    () =>
      computeGhostTextLines({
        promptCompletionText: completion.promptCompletion.text,
        bufferText: buffer.text,
        bufferLines: buffer.lines,
        cursor: buffer.cursor,
        inputWidth,
      }),
    [
      completion.promptCompletion.text,
      buffer.text,
      buffer.lines,
      buffer.cursor,
      inputWidth,
    ],
  );

  const scrollableData = useMemo(() => {
    const items: ScrollableItem[] = buffer.allVisualLines.map(
      (lineText, index) => ({
        type: 'visualLine',
        lineText,
        absoluteVisualIdx: index,
      }),
    );

    additionalLines.forEach((ghostLine, index) => {
      items.push({
        type: 'ghostLine',
        ghostLine,
        index,
      });
    });

    return items;
  }, [buffer.allVisualLines, additionalLines]);

  const renderItem = useCallback(
    ({ item }: { item: ScrollableItem; index: number }) => (
      <InputPromptLine
        item={item}
        buffer={buffer}
        focus={focus}
        showCursor={showCursor}
        cursorVisualRowAbsolute={cursorVisualRowAbsolute}
        cursorVisualColAbsolute={cursorVisualColAbsolute}
        inlineGhost={inlineGhost}
        inputWidth={inputWidth}
      />
    ),
    [
      buffer,
      focus,
      showCursor,
      cursorVisualRowAbsolute,
      cursorVisualColAbsolute,
      inlineGhost,
      inputWidth,
    ],
  );

  const useBackgroundColor = config.getUseBackgroundColor();

  const prevCursorRef = useRef(buffer.visualCursor);
  const prevTextRef = useRef(buffer.text);

  useEffect(() => {
    const cursorChanged = prevCursorRef.current !== buffer.visualCursor;
    const textChanged = prevTextRef.current !== buffer.text;

    prevCursorRef.current = buffer.visualCursor;
    prevTextRef.current = buffer.text;

    if (!cursorChanged && !textChanged) return;

    if (!listRef.current || !focus) return;
    const { scrollTop, innerHeight } = listRef.current.getScrollState();
    if (innerHeight === 0) return;

    const cursorVisualRow = buffer.visualCursor[0];
    const actualScrollTop = Math.round(scrollTop);

    if (
      cursorVisualRow < actualScrollTop ||
      cursorVisualRow >= actualScrollTop + innerHeight
    ) {
      let newScrollTop = actualScrollTop;
      if (cursorVisualRow < actualScrollTop) {
        newScrollTop = cursorVisualRow;
      } else if (cursorVisualRow >= actualScrollTop + innerHeight) {
        newScrollTop = cursorVisualRow - innerHeight + 1;
      }

      listRef.current.scrollToIndex({ index: newScrollTop });
    }
  }, [buffer.visualCursor, buffer.text, focus]);

  const listBackgroundColor = !useBackgroundColor
    ? undefined
    : theme.background.input;

  const useLineFallback = !!process.env['NO_COLOR'];

  useEffect(() => {
    if (onSuggestionsVisibilityChange) {
      onSuggestionsVisibilityChange(shouldShowSuggestions);
    }
  }, [shouldShowSuggestions, onSuggestionsVisibilityChange]);

  const { statusColor, statusText, showYoloStyling } =
    getInputPromptStatusPresentation(shellModeActive, approvalMode);

  const suggestionsNode = shouldShowSuggestions ? (
    <Box paddingRight={2}>
      <SuggestionsDisplay
        suggestions={activeCompletion.suggestions}
        activeIndex={activeCompletion.activeSuggestionIndex}
        isLoading={activeCompletion.isLoadingSuggestions}
        width={suggestionsWidth}
        scrollOffset={activeCompletion.visibleStartIndex}
        userInput={buffer.text}
        mode={
          completion.completionMode === CompletionMode.AT ||
          completion.completionMode === CompletionMode.SHELL
            ? 'reverse'
            : buffer.text.startsWith('/') &&
                !reverseSearchActive &&
                !commandSearchActive
              ? 'slash'
              : 'reverse'
        }
        expandedIndex={expandedSuggestionIndex}
      />
    </Box>
  ) : null;

  const borderColor =
    isShellFocused && !isEmbeddedShellFocused
      ? (statusColor ?? theme.ui.focus)
      : theme.border.default;

  return (
    <>
      {suggestionsPosition === 'above' && suggestionsNode}
      {useLineFallback || !useBackgroundColor ? (
        <Box
          borderStyle="round"
          borderTop={true}
          borderBottom={false}
          borderLeft={false}
          borderRight={false}
          borderColor={borderColor}
          width={promptContainerWidth}
          flexDirection="row"
          alignItems="flex-start"
          height={0}
        />
      ) : null}
      <HalfLinePaddedBox
        backgroundBaseColor={theme.background.input}
        backgroundOpacity={1}
        useBackgroundColor={useBackgroundColor}
      >
        <Box flexGrow={1} flexDirection="row" paddingX={1}>
          {isVoiceModeEnabled &&
            (isRecording ? (
              <ListeningIndicator color={theme.text.accent} />
            ) : (
              <Text color={theme.text.accent}>🎤 </Text>
            ))}
          <Text
            color={statusColor ?? theme.text.accent}
            aria-label={statusText || SCREEN_READER_USER_PREFIX}
          >
            {shellModeActive ? (
              reverseSearchActive ? (
                <Text
                  color={theme.text.link}
                  aria-label={SCREEN_READER_REVERSE_SEARCH_PREFIX}
                >
                  (r:){' '}
                </Text>
              ) : (
                '!'
              )
            ) : commandSearchActive ? (
              <Text
                color={theme.text.accent}
                aria-label={SCREEN_READER_REVERSE_SEARCH_PREFIX}
              >
                (r:){' '}
              </Text>
            ) : showYoloStyling ? (
              '*'
            ) : (
              '>'
            )}{' '}
          </Text>
          <Box flexGrow={1} flexDirection="column" ref={innerBoxRef}>
            {buffer.text.length === 0 && effectivePlaceholder ? (
              showCursor ? (
                <Text
                  terminalCursorFocus={showCursor}
                  terminalCursorPosition={0}
                >
                  {chalk.inverse(cpSlice(effectivePlaceholder, 0, 1))}
                  <Text color={theme.text.secondary}>
                    {cpSlice(effectivePlaceholder, 1)}
                  </Text>
                </Text>
              ) : (
                <Text color={theme.text.secondary}>{effectivePlaceholder}</Text>
              )
            ) : (
              <Box
                flexDirection="column"
                height={Math.min(buffer.viewportHeight, scrollableData.length)}
                width="100%"
              >
                {config.getUseTerminalBuffer() ? (
                  <ScrollableList
                    ref={listRef}
                    hasFocus={focus}
                    data={scrollableData}
                    renderItem={renderItem}
                    estimatedItemHeight={() => 1}
                    fixedItemHeight={true}
                    keyExtractor={(item) =>
                      item.type === 'visualLine'
                        ? `line-${item.absoluteVisualIdx}`
                        : `ghost-${item.index}`
                    }
                    width={inputWidth + SCROLLBAR_GUTTER_WIDTH}
                    backgroundColor={listBackgroundColor}
                    containerHeight={Math.min(
                      buffer.viewportHeight,
                      scrollableData.length,
                    )}
                  />
                ) : (
                  scrollableData
                    .slice(
                      buffer.visualScrollRow,
                      buffer.visualScrollRow + buffer.viewportHeight,
                    )
                    .map((item, index) => {
                      const actualIndex = buffer.visualScrollRow + index;
                      const key =
                        item.type === 'visualLine'
                          ? `line-${item.absoluteVisualIdx}`
                          : `ghost-${item.index}`;
                      return (
                        <Fragment key={key}>
                          {renderItem({ item, index: actualIndex })}
                        </Fragment>
                      );
                    })
                )}
              </Box>
            )}
          </Box>
        </Box>
      </HalfLinePaddedBox>
      {useLineFallback || !useBackgroundColor ? (
        <Box
          borderStyle="round"
          borderTop={false}
          borderBottom={true}
          borderLeft={false}
          borderRight={false}
          borderColor={borderColor}
          width={promptContainerWidth}
          flexDirection="row"
          alignItems="flex-start"
          height={0}
        />
      ) : null}
      {suggestionsPosition === 'below' && suggestionsNode}
    </>
  );
};
