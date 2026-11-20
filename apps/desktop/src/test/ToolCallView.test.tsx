import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ToolCallView } from '../features/chat/ToolCallView';
import type { ToolCallItem } from '../stores/AppContext';

describe('ToolCallView Component', () => {
  it('renders tool name and completed status', () => {
    const toolCall: ToolCallItem = {
      id: 'call-1',
      name: 'read_file',
      arguments: '{"path": "Cargo.toml"}',
      output: '[package]\nname = "test"',
      status: 'success',
    };

    render(<ToolCallView toolCall={toolCall} />);
    expect(screen.getByText('read_file')).toBeInTheDocument();
    expect(screen.getByText('Completed')).toBeInTheDocument();
  });

  it('renders running and failed states', () => {
    const { rerender } = render(
      <ToolCallView
        toolCall={{
          id: 'call-running',
          name: 'exec_command',
          arguments: '{"command": "cargo test"}',
          status: 'running',
        }}
      />
    );

    expect(screen.getByText('Executing')).toBeInTheDocument();

    rerender(
      <ToolCallView
        toolCall={{
          id: 'call-failed',
          name: 'exec_command',
          arguments: '{"command": "cargo test"}',
          output: 'error: test failed',
          status: 'failed',
        }}
      />
    );

    expect(screen.getByText('Failed')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /exec_command/i }));
    expect(screen.getByText('error: test failed')).toBeInTheDocument();
  });

  it('expands arguments and output on click', () => {
    const toolCall: ToolCallItem = {
      id: 'call-2',
      name: 'execute_command',
      arguments: '{"cmd": "cargo test"}',
      output: 'test result: ok',
      status: 'success',
    };

    render(<ToolCallView toolCall={toolCall} />);
    expect(screen.queryByText('Arguments:')).not.toBeInTheDocument();

    const toggle = screen.getByRole('button', { name: /execute_command/i });
    fireEvent.click(toggle);
    expect(toggle).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByText('Arguments:')).toBeInTheDocument();
    expect(screen.getByText(/cargo test/)).toBeInTheDocument();
    expect(screen.getByText('Output:')).toBeInTheDocument();
    expect(screen.getByText('test result: ok')).toBeInTheDocument();

    fireEvent.click(toggle);
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByText('Arguments:')).not.toBeInTheDocument();
  });
});
