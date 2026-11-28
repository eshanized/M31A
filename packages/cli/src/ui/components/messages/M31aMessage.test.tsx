/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { M31aMessage } from './M31aMessage.js';
import { StreamingState } from '../../types.js';
import { renderWithProviders } from '../../../test-utils/render.js';

describe('<M31aMessage /> - Raw Markdown Display Snapshots', () => {
  const baseProps = {
    text: 'Test **bold** and `code` markdown\n\n```javascript\nconst x = 1;\n```',
    isPending: false,
    terminalWidth: 80,
  };

  it.each([
    { renderMarkdown: true, description: '(default)' },
    {
      renderMarkdown: false,
      description: '(raw markdown with syntax highlighting, no line numbers)',
    },
  ])(
    'renders with renderMarkdown=$renderMarkdown $description',
    async ({ renderMarkdown }) => {
      const { lastFrame, unmount } = await renderWithProviders(
        <M31aMessage {...baseProps} />,
        {
          uiState: { renderMarkdown, streamingState: StreamingState.Idle },
        },
      );
      expect(lastFrame()).toMatchSnapshot();
      unmount();
    },
  );

  it.each([{ renderMarkdown: true }, { renderMarkdown: false }])(
    'renders pending state with renderMarkdown=$renderMarkdown',
    async ({ renderMarkdown }) => {
      const { lastFrame, unmount } = await renderWithProviders(
        <M31aMessage {...baseProps} isPending={true} />,
        {
          uiState: { renderMarkdown, streamingState: StreamingState.Idle },
        },
      );
      expect(lastFrame()).toMatchSnapshot();
      unmount();
    },
  );

  it('wraps long lines correctly in raw markdown mode', async () => {
    const terminalWidth = 20;
    const text =
      'This is a long line that should wrap correctly without truncation';
    const { lastFrame, unmount } = await renderWithProviders(
      <M31aMessage
        text={text}
        isPending={false}
        terminalWidth={terminalWidth}
      />,
      {
        uiState: { renderMarkdown: false, streamingState: StreamingState.Idle },
      },
    );
    expect(lastFrame()).toMatchSnapshot();
    unmount();
  });
});
