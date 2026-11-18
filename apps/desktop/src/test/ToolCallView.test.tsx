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

    fireEvent.click(screen.getByText('execute_command'));
    expect(screen.getByText('Arguments:')).toBeInTheDocument();
    expect(screen.getByText('Output:')).toBeInTheDocument();
  });
});
