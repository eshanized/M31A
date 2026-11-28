/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type React from 'react';
import type { DOMElement } from 'ink';
import type { ScrollableListRef } from '../shared/ScrollableList.js';
import {
  type TextBuffer,
  getTransformUnderCursor,
} from '../shared/text-buffer.js';
import { getCachedStringWidth } from '../../utils/textUtils.js';
import { useMouseClick } from '../../hooks/useMouseClick.js';
import { useMouse, type MouseEvent } from '../../contexts/MouseContext.js';
import type { ScrollableItem } from './inputPromptUtils.js';

export interface UseInputPromptMouseOptions {
  innerBoxRef: React.RefObject<DOMElement | null>;
  listRef: React.RefObject<ScrollableListRef<ScrollableItem> | null>;
  buffer: TextBuffer;
  focus: boolean;
  copyModeEnabled?: boolean;
  isEmbeddedShellFocused?: boolean;
  isAlternateBuffer: boolean;
  setSuppressCompletion: (value: boolean) => void;
  setEmbeddedShellFocused: (value: boolean) => void;
  handleClipboardPaste: () => Promise<void>;
}

/**
 * Registers mouse interactions for cursor positioning, paste placeholder toggling,
 * and right-click clipboard paste while respecting copy-mode and focus rules.
 */
export function useInputPromptMouse({
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
}: UseInputPromptMouseOptions): void {
  const isMouseActive = focus && !copyModeEnabled;

  useMouseClick(
    innerBoxRef,
    (_event, relX, relY) => {
      setSuppressCompletion(true);
      if (isEmbeddedShellFocused) {
        setEmbeddedShellFocused(false);
      }
      const currentScrollTop = Math.round(
        listRef.current?.getScrollState().scrollTop ?? buffer.visualScrollRow,
      );
      const visualRow = currentScrollTop + relY;
      buffer.moveToVisualPosition(visualRow, relX);
    },
    { isActive: isMouseActive },
  );

  // Double-click to expand/collapse paste placeholders
  useMouseClick(
    innerBoxRef,
    (_event, relX, relY) => {
      if (!isAlternateBuffer) return;

      const currentScrollTop = Math.round(
        listRef.current?.getScrollState().scrollTop ?? buffer.visualScrollRow,
      );
      const visualLine = buffer.allVisualLines[currentScrollTop + relY];
      if (!visualLine) return;

      // Even if we click past the end of the line, we might want to collapse an expanded paste
      const isPastEndOfLine = relX >= getCachedStringWidth(visualLine);

      const logicalPos = isPastEndOfLine
        ? null
        : buffer.getLogicalPositionFromVisual(currentScrollTop + relY, relX);

      // Check for paste placeholder (collapsed state)
      if (logicalPos) {
        const transform = getTransformUnderCursor(
          logicalPos.row,
          logicalPos.col,
          buffer.transformationsByLine,
          { includeEdge: true },
        );
        if (transform?.type === 'paste' && transform.id) {
          buffer.togglePasteExpansion(
            transform.id,
            logicalPos.row,
            logicalPos.col,
          );
          return;
        }
      }

      // If we didn't click a placeholder to expand, check if we are inside or after
      // an expanded paste region and collapse it.
      const visualRow = currentScrollTop + relY;
      const mapEntry = buffer.visualToLogicalMap[visualRow];
      const row = mapEntry ? mapEntry[0] : visualRow;
      const expandedId = buffer.getExpandedPasteAtLine(row);
      if (expandedId) {
        buffer.togglePasteExpansion(
          expandedId,
          row,
          logicalPos?.col ?? relX, // Fallback to relX if past end of line
        );
      }
    },
    { isActive: isMouseActive, name: 'double-click' },
  );

  useMouse(
    (event: MouseEvent) => {
      if (event.name === 'right-release') {
        setSuppressCompletion(false);
        // eslint-disable-next-line @typescript-eslint/no-floating-promises
        handleClipboardPaste();
      }
    },
    { isActive: isMouseActive },
  );
}
