import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { CodeBlock } from '../features/chat/CodeBlock';

describe('CodeBlock Component', () => {
  it('renders code and language badge', () => {
    render(<CodeBlock language="rust" code="fn main() {}" />);
    expect(screen.getByText('rust')).toBeInTheDocument();
    expect(screen.getByText('fn main() {}')).toBeInTheDocument();
  });

  it('handles copy button click', async () => {
    Object.assign(navigator, {
      clipboard: {
        writeText: vi.fn().mockResolvedValue(undefined),
      },
    });

    render(<CodeBlock language="typescript" code="const x = 42;" />);
    const copyButton = screen.getByRole('button', { name: /copy/i });
    await act(async () => {
      fireEvent.click(copyButton);
    });

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('const x = 42;');
    expect(screen.getByRole('button', { name: /copied/i })).toBeInTheDocument();
  });
});
