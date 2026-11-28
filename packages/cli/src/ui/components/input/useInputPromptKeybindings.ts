/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import { useCallback, useRef } from 'react';
import { type TextBuffer, logicalPosToOffset } from '../shared/text-buffer.js';
import { MAX_WIDTH } from '../SuggestionsDisplay.js';
import { escapeAtSymbols } from '../../hooks/atCommandProcessor.js';
import type { UseCommandCompletionReturn } from '../../hooks/useCommandCompletion.js';
import type { UseInputHistoryReturn } from '../../hooks/useInputHistory.js';
import type { UseShellHistoryReturn } from '../../hooks/useShellHistory.js';
import type { UseReverseSearchCompletionReturn } from '../../hooks/useReverseSearchCompletion.js';
import { useKeypress, type Key } from '../../hooks/useKeypress.js';
import { Command, type KeyMatchers } from '../../key/keyMatchers.js';
import { formatCommand } from '../../key/keybindingUtils.js';
import { cpSlice, cpLen } from '../../utils/textUtils.js';
import {
  isAutoExecutableCommand,
  isSlashCommand,
} from '../../utils/commandUtils.js';
import {
  appEvents,
  AppEvent,
  TransientMessageType,
} from '../../../utils/events.js';
import { StreamingState } from '../../types.js';
import type { VimMode } from '../../contexts/VimModeContext.js';
import type { MergedSettings } from '../../../config/settings.js';
import {
  isLargePaste,
  isTerminalPasteTrusted,
  tryTogglePasteExpansion,
} from './inputPromptUtils.js';

export interface UseInputPromptKeybindingsOptions {
  focus: boolean;
  isEmbeddedShellFocused?: boolean;
  copyModeEnabled?: boolean;
  buffer: TextBuffer;
  completion: UseCommandCompletionReturn;
  shellModeActive: boolean;
  setShellModeActive: (value: boolean) => void;
  onClearScreen: () => void;
  inputHistory: UseInputHistoryReturn;
  handleSubmit: (submittedValue: string) => void;
  shellHistory: UseShellHistoryReturn;
  reverseSearchCompletion: UseReverseSearchCompletionReturn;
  commandSearchCompletion: UseReverseSearchCompletionReturn;
  handleClipboardPaste: () => Promise<void>;
  resetCompletionState: () => void;
  resetEscapeState: () => void;
  vimHandleInput?: (key: Key) => boolean;
  vimEnabled?: boolean;
  vimMode?: VimMode;
  reverseSearchActive: boolean;
  setReverseSearchActive: (active: boolean) => void;
  commandSearchActive: boolean;
  setCommandSearchActive: (active: boolean) => void;
  textBeforeReverseSearch: string;
  setTextBeforeReverseSearch: (text: string) => void;
  cursorPosition: [number, number];
  setCursorPosition: (pos: [number, number]) => void;
  recentUnsafePasteTime: number | null;
  setRecentUnsafePasteTime: (time: number | null) => void;
  pasteTimeoutRef: React.MutableRefObject<NodeJS.Timeout | null>;
  kittyProtocolEnabled: boolean;
  shortcutsHelpVisible: boolean;
  setShortcutsHelpVisible: (visible: boolean) => void;
  tryLoadQueuedMessages: () => boolean;
  onQueueMessage?: (message: string) => void;
  setQueueErrorMessage: (message: string | null) => void;
  resetReverseSearchCompletionState: () => void;
  setBannerVisible: (visible: boolean) => void;
  activePtyId?: number | null;
  setEmbeddedShellFocused: (focused: boolean) => void;
  backgroundTaskCount: number;
  backgroundTaskHeight: number;
  streamingState: StreamingState;
  handleEscPress: () => void;
  resetTurnBaseline: () => void;
  registerPlainTabPress: () => number;
  resetPlainTabPress: () => void;
  toggleCleanUiDetailsVisible: () => void;
  shouldShowSuggestions: boolean;
  isShellSuggestionsVisible: boolean;
  forceShowShellSuggestions: boolean;
  setForceShowShellSuggestions: (value: boolean) => void;
  setSuppressCompletion: (value: boolean) => void;
  setExpandedSuggestionIndex: (index: number) => void;
  hasUserNavigatedSuggestions: React.MutableRefObject<boolean>;
  keyMatchers: KeyMatchers;
  isHelpDismissKey: (key: Key) => boolean;
  settings: MergedSettings;
  handleVoiceInput: (key: Key) => boolean;
}

const IME_COMPOSITION_FLUSH_MS = 25;
const CJK_KANA_HANGUL_RE =
  /[\u3000-\u30ff\u4e00-\u9fff\uac00-\ud7af\uff00-\uffef]/;

function isImeBurstSequence(sequence: string): boolean {
  if (CJK_KANA_HANGUL_RE.test(sequence)) {
    return true;
  }
  let nonAsciiCount = 0;
  for (let i = 0; i < sequence.length; i++) {
    if (sequence.charCodeAt(i) > 0x7f) {
      nonAsciiCount++;
      if (nonAsciiCount >= 2) {
        return true;
      }
    }
  }
  return false;
}

/**
 * Registers the primary prompt keybinding handler, coordinating voice input,
 * reverse search, completion menus, history navigation, paste guards, and shell toggles.
 */
export function useInputPromptKeybindings(
  opts: UseInputPromptKeybindingsOptions,
): void {
  const lastImeCommitTimeRef = useRef<number>(0);
  const {
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
    kittyProtocolEnabled,
    shortcutsHelpVisible,
    setShortcutsHelpVisible,
    tryLoadQueuedMessages,
    onQueueMessage,
    setQueueErrorMessage,
    resetReverseSearchCompletionState,
    setBannerVisible,
    activePtyId,
    setEmbeddedShellFocused,
    backgroundTaskCount,
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
  } = opts;

  const handleInput = useCallback(
    (key: Key) => {
      if (!focus) {
        return false;
      }

      if (handleVoiceInput(key)) return true;

      if (
        key.sequence &&
        !key.ctrl &&
        !key.alt &&
        !key.cmd &&
        !keyMatchers[Command.SUBMIT](key) &&
        !keyMatchers[Command.RETURN](key) &&
        isImeBurstSequence(key.sequence)
      ) {
        lastImeCommitTimeRef.current = Date.now();
      }

      // Determine if this keypress is a history navigation command
      const isHistoryUp =
        !shellModeActive &&
        (keyMatchers[Command.HISTORY_UP](key) ||
          (keyMatchers[Command.NAVIGATION_UP](key) &&
            (buffer.allVisualLines.length === 1 ||
              (buffer.visualCursor[0] === 0 && buffer.visualScrollRow === 0))));
      const isHistoryDown =
        !shellModeActive &&
        (keyMatchers[Command.HISTORY_DOWN](key) ||
          (keyMatchers[Command.NAVIGATION_DOWN](key) &&
            (buffer.allVisualLines.length === 1 ||
              buffer.visualCursor[0] === buffer.allVisualLines.length - 1)));

      const isHistoryNav = isHistoryUp || isHistoryDown;
      const isCursorMovement =
        keyMatchers[Command.MOVE_LEFT](key) ||
        keyMatchers[Command.MOVE_RIGHT](key) ||
        keyMatchers[Command.MOVE_UP](key) ||
        keyMatchers[Command.MOVE_DOWN](key) ||
        keyMatchers[Command.MOVE_WORD_LEFT](key) ||
        keyMatchers[Command.MOVE_WORD_RIGHT](key) ||
        keyMatchers[Command.HOME](key) ||
        keyMatchers[Command.END](key);

      const isSuggestionsNav =
        shouldShowSuggestions &&
        (keyMatchers[Command.COMPLETION_UP](key) ||
          keyMatchers[Command.COMPLETION_DOWN](key) ||
          keyMatchers[Command.EXPAND_SUGGESTION](key) ||
          keyMatchers[Command.COLLAPSE_SUGGESTION](key) ||
          keyMatchers[Command.ACCEPT_SUGGESTION](key));

      // Reset completion suppression if the user performs any action other than
      // history navigation or cursor movement.
      // We explicitly skip this if we are currently navigating suggestions.
      if (!isSuggestionsNav) {
        setSuppressCompletion(
          isHistoryNav || isCursorMovement || keyMatchers[Command.ESCAPE](key),
        );
        hasUserNavigatedSuggestions.current = false;

        if (key.name !== 'tab') {
          setForceShowShellSuggestions(false);
        }
      }

      // Handle escape to close shortcuts panel first, before letting it bubble
      // up for cancellation. This ensures pressing Escape once closes the panel,
      // and pressing again cancels the operation.
      if (shortcutsHelpVisible && key.name === 'escape') {
        setShortcutsHelpVisible(false);
        return true;
      }

      const isGenerating =
        streamingState === StreamingState.Responding ||
        streamingState === StreamingState.WaitingForConfirmation;

      const isQueueMessageKey = keyMatchers[Command.QUEUE_MESSAGE](key);
      const isPlainTab =
        key.name === 'tab' && !key.shift && !key.alt && !key.ctrl && !key.cmd;
      const hasTabCompletionInteraction =
        (completion.showSuggestions && isShellSuggestionsVisible) ||
        Boolean(completion.promptCompletion.text) ||
        reverseSearchActive ||
        commandSearchActive;

      if (
        isGenerating &&
        isQueueMessageKey &&
        !hasTabCompletionInteraction &&
        buffer.text.trim().length > 0
      ) {
        const trimmedMessage = buffer.text.trim();
        const isSlash = isSlashCommand(trimmedMessage);

        if (isSlash || shellModeActive) {
          setQueueErrorMessage(
            `${shellModeActive ? 'Shell' : 'Slash'} commands cannot be queued`,
          );
        } else if (onQueueMessage) {
          onQueueMessage(buffer.text);
          buffer.setText('');
          resetCompletionState();
          resetReverseSearchCompletionState();
        }
        resetPlainTabPress();
        return true;
      }

      if (isPlainTab && shellModeActive) {
        resetPlainTabPress();
        if (!shouldShowSuggestions) {
          setSuppressCompletion(false);
          if (completion.promptCompletion.text) {
            completion.promptCompletion.accept();
            return true;
          } else if (
            completion.suggestions.length > 0 &&
            !forceShowShellSuggestions
          ) {
            setForceShowShellSuggestions(true);
            return true;
          }
        }
      } else if (isPlainTab) {
        if (!hasTabCompletionInteraction) {
          if (registerPlainTabPress() === 2) {
            toggleCleanUiDetailsVisible();
            resetPlainTabPress();
            return true;
          }
        } else {
          resetPlainTabPress();
        }
      } else {
        resetPlainTabPress();
      }

      if (key.name === 'paste') {
        if (shortcutsHelpVisible) {
          setShortcutsHelpVisible(false);
        }
        // Record paste time to prevent accidental auto-submission
        if (!isTerminalPasteTrusted(kittyProtocolEnabled)) {
          setRecentUnsafePasteTime(Date.now());

          // Clear any existing paste timeout
          if (pasteTimeoutRef.current) {
            clearTimeout(pasteTimeoutRef.current);
          }

          // Clear the paste protection after a very short delay to prevent
          // false positives.
          pasteTimeoutRef.current = setTimeout(() => {
            setRecentUnsafePasteTime(null);
            pasteTimeoutRef.current = null;
          }, 40);
        }
        if (settings.ui?.escapePastedAtSymbols) {
          buffer.handleInput({
            ...key,
            sequence: escapeAtSymbols(key.sequence || ''),
          });
        } else {
          buffer.handleInput(key);
        }

        if (key.sequence && isLargePaste(key.sequence)) {
          appEvents.emit(AppEvent.TransientMessage, {
            message: `Press ${formatCommand(Command.EXPAND_PASTE)} to expand pasted text`,
            type: TransientMessageType.Hint,
          });
        }
        return true;
      }

      if (shortcutsHelpVisible && isHelpDismissKey(key)) {
        setShortcutsHelpVisible(false);
      }

      if (shortcutsHelpVisible) {
        if (
          key.sequence === '?' &&
          key.insertable &&
          (!vimEnabled || vimMode === 'INSERT')
        ) {
          setShortcutsHelpVisible(false);
          buffer.handleInput(key);
          return true;
        }
        // Escape is handled earlier to ensure it closes the panel before
        // potentially cancelling an operation
        if (key.name === 'backspace' || key.sequence === '\b') {
          setShortcutsHelpVisible(false);
          return true;
        }
        if (key.insertable) {
          setShortcutsHelpVisible(false);
        }
      }

      if (
        key.sequence === '?' &&
        key.insertable &&
        !shortcutsHelpVisible &&
        buffer.text.length === 0 &&
        (!vimEnabled || vimMode === 'INSERT')
      ) {
        setShortcutsHelpVisible(true);
        return true;
      }

      if (vimHandleInput && vimHandleInput(key)) {
        return true;
      }

      // Reset ESC count and hide prompt on any non-ESC key
      if (key.name !== 'escape') {
        resetEscapeState();
      }

      // Ctrl+O to expand/collapse paste placeholders
      if (keyMatchers[Command.EXPAND_PASTE](key)) {
        const handled = tryTogglePasteExpansion(buffer);
        if (handled) return true;
      }

      if (
        key.sequence === '!' &&
        buffer.text === '' &&
        !(completion.showSuggestions && isShellSuggestionsVisible)
      ) {
        setShellModeActive(!shellModeActive);
        buffer.setText(''); // Clear the '!' from input
        resetTurnBaseline();
        return true;
      }

      if (keyMatchers[Command.ESCAPE](key)) {
        const cancelSearch = (
          setActive: (active: boolean) => void,
          resetCompletion: () => void,
        ) => {
          setActive(false);
          resetCompletion();
          buffer.setText(textBeforeReverseSearch);
          const offset = logicalPosToOffset(
            buffer.lines,
            cursorPosition[0],
            cursorPosition[1],
          );
          buffer.moveToOffset(offset);
          setExpandedSuggestionIndex(-1);
        };

        if (reverseSearchActive) {
          cancelSearch(
            setReverseSearchActive,
            reverseSearchCompletion.resetCompletionState,
          );
          return true;
        }
        if (commandSearchActive) {
          cancelSearch(
            setCommandSearchActive,
            commandSearchCompletion.resetCompletionState,
          );
          return true;
        }

        if (completion.showSuggestions && isShellSuggestionsVisible) {
          completion.resetCompletionState();
          setExpandedSuggestionIndex(-1);
          resetEscapeState();
          return true;
        }

        if (shellModeActive) {
          setShellModeActive(false);
          resetEscapeState();
          return true;
        }

        // If we're generating and no local overlay consumed Escape, let it
        // propagate to the global cancellation handler.
        if (isGenerating) {
          return false;
        }

        handleEscPress();
        return true;
      }

      if (keyMatchers[Command.CLEAR_SCREEN](key)) {
        setBannerVisible(false);
        onClearScreen();
        return true;
      }

      if (shellModeActive && keyMatchers[Command.REVERSE_SEARCH](key)) {
        setReverseSearchActive(true);
        setTextBeforeReverseSearch(buffer.text);
        setCursorPosition(buffer.cursor);
        return true;
      }

      if (reverseSearchActive || commandSearchActive) {
        const isCommandSearch = commandSearchActive;

        const sc = isCommandSearch
          ? commandSearchCompletion
          : reverseSearchCompletion;

        const {
          activeSuggestionIndex,
          navigateUp,
          navigateDown,
          showSuggestions,
          suggestions,
        } = sc;
        const setActive = isCommandSearch
          ? setCommandSearchActive
          : setReverseSearchActive;
        const resetState = sc.resetCompletionState;

        if (showSuggestions) {
          if (keyMatchers[Command.NAVIGATION_UP](key)) {
            navigateUp();
            return true;
          }
          if (keyMatchers[Command.NAVIGATION_DOWN](key)) {
            navigateDown();
            return true;
          }
          if (keyMatchers[Command.COLLAPSE_SUGGESTION](key)) {
            if (suggestions[activeSuggestionIndex].value.length >= MAX_WIDTH) {
              setExpandedSuggestionIndex(-1);
              return true;
            }
          }
          if (keyMatchers[Command.EXPAND_SUGGESTION](key)) {
            if (suggestions[activeSuggestionIndex].value.length >= MAX_WIDTH) {
              setExpandedSuggestionIndex(activeSuggestionIndex);
              return true;
            }
          }
          if (keyMatchers[Command.ACCEPT_SUGGESTION_REVERSE_SEARCH](key)) {
            sc.handleAutocomplete(activeSuggestionIndex);
            resetState();
            setActive(false);
            return true;
          }
        }

        if (keyMatchers[Command.SUBMIT_REVERSE_SEARCH](key)) {
          const textToSubmit =
            showSuggestions && activeSuggestionIndex > -1
              ? suggestions[activeSuggestionIndex].value
              : buffer.text;
          handleSubmit(textToSubmit);
          resetState();
          setActive(false);
          return true;
        }

        // Prevent up/down from falling through to regular history navigation
        if (
          keyMatchers[Command.NAVIGATION_UP](key) ||
          keyMatchers[Command.NAVIGATION_DOWN](key)
        ) {
          return true;
        }
      }

      // If the command is a perfect match, pressing enter should execute it.
      // We prioritize execution unless the user is explicitly selecting a different suggestion.
      if (
        completion.isPerfectMatch &&
        keyMatchers[Command.SUBMIT](key) &&
        recentUnsafePasteTime === null &&
        (!(completion.showSuggestions && isShellSuggestionsVisible) ||
          (completion.activeSuggestionIndex <= 0 &&
            !hasUserNavigatedSuggestions.current))
      ) {
        handleSubmit(buffer.text);
        return true;
      }

      // Newline insertion
      if (keyMatchers[Command.NEWLINE](key)) {
        buffer.newline();
        return true;
      }

      if (completion.showSuggestions && isShellSuggestionsVisible) {
        if (completion.suggestions.length > 1) {
          if (keyMatchers[Command.COMPLETION_UP](key)) {
            completion.navigateUp();
            hasUserNavigatedSuggestions.current = true;
            setExpandedSuggestionIndex(-1); // Reset expansion when navigating
            return true;
          }
          if (keyMatchers[Command.COMPLETION_DOWN](key)) {
            completion.navigateDown();
            hasUserNavigatedSuggestions.current = true;
            setExpandedSuggestionIndex(-1); // Reset expansion when navigating
            return true;
          }
        }

        if (keyMatchers[Command.ACCEPT_SUGGESTION](key)) {
          if (completion.suggestions.length > 0) {
            const targetIndex =
              completion.activeSuggestionIndex === -1
                ? 0 // Default to the first if none is active
                : completion.activeSuggestionIndex;

            if (targetIndex < completion.suggestions.length) {
              const suggestion = completion.suggestions[targetIndex];

              const isEnterKey = key.name === 'enter' && !key.ctrl;

              if (isEnterKey && shellModeActive) {
                if (hasUserNavigatedSuggestions.current) {
                  completion.handleAutocomplete(
                    completion.activeSuggestionIndex,
                  );
                  setExpandedSuggestionIndex(-1);
                  hasUserNavigatedSuggestions.current = false;
                  return true;
                }
                completion.resetCompletionState();
                setExpandedSuggestionIndex(-1);
                hasUserNavigatedSuggestions.current = false;
                if (buffer.text.trim()) {
                  handleSubmit(buffer.text);
                }
                return true;
              }

              if (isEnterKey && buffer.text.startsWith('/')) {
                if (suggestion.submitValue) {
                  setExpandedSuggestionIndex(-1);
                  handleSubmit(suggestion.submitValue.trim());
                  return true;
                }

                const { isArgumentCompletion, leafCommand } =
                  completion.slashCompletionRange;

                if (
                  isArgumentCompletion &&
                  isAutoExecutableCommand(leafCommand)
                ) {
                  const completedText = completion.getCompletedText(suggestion);
                  if (completedText) {
                    setExpandedSuggestionIndex(-1);
                    handleSubmit(completedText.trim());
                    return true;
                  }
                } else if (!isArgumentCompletion) {
                  const command =
                    completion.getCommandFromSuggestion(suggestion);

                  // Only auto-execute if the command has no completion function
                  // (i.e., it doesn't require an argument to be selected)
                  if (
                    command &&
                    isAutoExecutableCommand(command) &&
                    !command.completion
                  ) {
                    const completedText =
                      completion.getCompletedText(suggestion);

                    if (completedText) {
                      setExpandedSuggestionIndex(-1);
                      handleSubmit(completedText.trim());
                      return true;
                    }
                  }
                }
              }

              // Default behavior: auto-complete to prompt box
              completion.handleAutocomplete(targetIndex);
              setExpandedSuggestionIndex(-1); // Reset expansion after selection
            }
          }
          return true;
        }
      }

      // Handle Tab key for ghost text acceptance
      if (
        key.name === 'tab' &&
        !key.shift &&
        !(completion.showSuggestions && isShellSuggestionsVisible) &&
        completion.promptCompletion.text
      ) {
        completion.promptCompletion.accept();
        return true;
      }

      if (!shellModeActive) {
        if (keyMatchers[Command.REVERSE_SEARCH](key)) {
          setCommandSearchActive(true);
          setTextBeforeReverseSearch(buffer.text);
          setCursorPosition(buffer.cursor);
          return true;
        }

        if (isHistoryUp) {
          if (
            keyMatchers[Command.NAVIGATION_UP](key) &&
            buffer.visualCursor[1] > 0
          ) {
            buffer.move('home');
            return true;
          }
          // Check for queued messages first when input is empty
          // If no queued messages, inputHistory.navigateUp() is called inside tryLoadQueuedMessages
          if (tryLoadQueuedMessages()) {
            return true;
          }
          // Only navigate history if popAllMessages doesn't exist
          inputHistory.navigateUp();
          return true;
        }
        if (isHistoryDown) {
          if (
            keyMatchers[Command.NAVIGATION_DOWN](key) &&
            buffer.visualCursor[1] <
              cpLen(buffer.allVisualLines[buffer.visualCursor[0]] || '')
          ) {
            buffer.move('end');
            return true;
          }
          inputHistory.navigateDown();
          return true;
        }
      } else {
        // Shell History Navigation
        if (keyMatchers[Command.NAVIGATION_UP](key)) {
          if (
            (buffer.allVisualLines.length === 1 ||
              (buffer.visualCursor[0] === 0 && buffer.visualScrollRow === 0)) &&
            buffer.visualCursor[1] > 0
          ) {
            buffer.move('home');
            return true;
          }
          const prevCommand = shellHistory.getPreviousCommand();
          if (prevCommand !== null) buffer.setText(prevCommand);
          return true;
        }
        if (keyMatchers[Command.NAVIGATION_DOWN](key)) {
          if (
            (buffer.allVisualLines.length === 1 ||
              buffer.visualCursor[0] === buffer.allVisualLines.length - 1) &&
            buffer.visualCursor[1] <
              cpLen(buffer.allVisualLines[buffer.visualCursor[0]] || '')
          ) {
            buffer.move('end');
            return true;
          }
          const nextCommand = shellHistory.getNextCommand();
          if (nextCommand !== null) buffer.setText(nextCommand);
          return true;
        }
      }

      if (keyMatchers[Command.SUBMIT](key)) {
        if (buffer.text.trim()) {
          const isRecentImeFlush =
            Date.now() - lastImeCommitTimeRef.current <
            IME_COMPOSITION_FLUSH_MS;
          // Check if a paste or IME composition commit occurred recently to prevent accidental auto-submission
          if (recentUnsafePasteTime !== null || isRecentImeFlush) {
            lastImeCommitTimeRef.current = 0;
            buffer.newline();
            return true;
          }

          const [row, col] = buffer.cursor;
          const line = buffer.lines[row];
          const charBefore = col > 0 ? cpSlice(line, col - 1, col) : '';
          if (charBefore === '\\') {
            buffer.backspace();
            buffer.newline();
          } else {
            handleSubmit(buffer.text);
          }
        }
        return true;
      }

      // Ctrl+A (Home) / Ctrl+E (End)
      if (keyMatchers[Command.HOME](key)) {
        buffer.move('home');
        return true;
      }
      if (keyMatchers[Command.END](key)) {
        buffer.move('end');
        return true;
      }

      // Kill line commands
      if (keyMatchers[Command.KILL_LINE_RIGHT](key)) {
        buffer.killLineRight();
        return true;
      }
      if (keyMatchers[Command.KILL_LINE_LEFT](key)) {
        buffer.killLineLeft();
        return true;
      }

      if (keyMatchers[Command.DELETE_WORD_BACKWARD](key)) {
        buffer.deleteWordLeft();
        return true;
      }

      // External editor
      if (keyMatchers[Command.OPEN_EXTERNAL_EDITOR](key)) {
        // eslint-disable-next-line @typescript-eslint/no-floating-promises
        buffer.openInExternalEditor();
        return true;
      }

      if (keyMatchers[Command.DEPRECATED_OPEN_EXTERNAL_EDITOR](key)) {
        const cmdKey = formatCommand(Command.OPEN_EXTERNAL_EDITOR);
        appEvents.emit(AppEvent.TransientMessage, {
          message: `Use ${cmdKey} to open the external editor.`,
          type: TransientMessageType.Hint,
        });
        return true;
      }

      // Ctrl+V for clipboard paste
      if (keyMatchers[Command.PASTE_CLIPBOARD](key)) {
        // eslint-disable-next-line @typescript-eslint/no-floating-promises
        handleClipboardPaste();
        return true;
      }

      if (keyMatchers[Command.TOGGLE_BACKGROUND_SHELL](key)) {
        return false;
      }

      if (keyMatchers[Command.FOCUS_SHELL_INPUT](key)) {
        if (
          activePtyId ||
          (backgroundTaskCount > 0 && backgroundTaskHeight > 0)
        ) {
          setEmbeddedShellFocused(true);
          return true;
        }
        return false;
      }

      // If we're generating and user presses Ctrl+C (QUIT), do not swallow it as
      // CLEAR_INPUT in the text buffer; let it propagate to cancel ongoing operations.
      if (isGenerating && keyMatchers[Command.QUIT](key)) {
        return false;
      }

      // Fall back to the text buffer's default input handling for all other keys
      const handled = buffer.handleInput(key);

      if (handled) {
        if (keyMatchers[Command.CLEAR_INPUT](key)) {
          resetCompletionState();
        }

        // Clear ghost text when user types regular characters (not navigation/control keys)
        if (
          completion.promptCompletion.text &&
          key.sequence &&
          key.sequence.length === 1 &&
          !key.alt &&
          !key.ctrl &&
          !key.cmd
        ) {
          completion.promptCompletion.clear();
          setExpandedSuggestionIndex(-1);
        }
      }
      return handled;
    },
    [
      focus,
      buffer,
      completion,
      setForceShowShellSuggestions,
      shellModeActive,
      setShellModeActive,
      onClearScreen,
      inputHistory,
      handleSubmit,
      shellHistory,
      reverseSearchCompletion,
      handleClipboardPaste,
      resetCompletionState,
      resetEscapeState,
      vimHandleInput,
      vimEnabled,
      vimMode,
      reverseSearchActive,
      setReverseSearchActive,
      textBeforeReverseSearch,
      setTextBeforeReverseSearch,
      cursorPosition,
      setCursorPosition,
      recentUnsafePasteTime,
      setRecentUnsafePasteTime,
      pasteTimeoutRef,
      commandSearchActive,
      setCommandSearchActive,
      commandSearchCompletion,
      kittyProtocolEnabled,
      shortcutsHelpVisible,
      setShortcutsHelpVisible,
      tryLoadQueuedMessages,
      onQueueMessage,
      setQueueErrorMessage,
      resetReverseSearchCompletionState,
      setBannerVisible,
      activePtyId,
      setEmbeddedShellFocused,
      backgroundTaskCount,
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
      setSuppressCompletion,
      setExpandedSuggestionIndex,
      hasUserNavigatedSuggestions,
      keyMatchers,
      isHelpDismissKey,
      settings,
      handleVoiceInput,
    ],
  );

  useKeypress(handleInput, {
    isActive: focus && !isEmbeddedShellFocused && !copyModeEnabled,
    priority: true,
  });
}
