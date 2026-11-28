/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect } from 'vitest';
import {
  detectMermaidDiagramKind,
  renderMermaidToLines,
} from './terminalMermaid.js';

describe('terminalMermaid', () => {
  it('detects supported mermaid diagram types', () => {
    expect(detectMermaidDiagramKind('flowchart TD\n  A --> B')).toBe(
      'flowchart',
    );
    expect(detectMermaidDiagramKind('sequenceDiagram\n  A->>B: hi')).toBe(
      'sequence',
    );
    expect(detectMermaidDiagramKind('stateDiagram-v2\n  [*] --> Ready')).toBe(
      'state',
    );
    expect(detectMermaidDiagramKind('timeline\n  2025 : Release')).toBe(
      'timeline',
    );
    expect(detectMermaidDiagramKind('gitGraph\n  commit')).toBe('gitGraph');
    expect(detectMermaidDiagramKind('gantt\n  title Plan\n  Task :a, 1d')).toBe(
      'gantt',
    );
    expect(detectMermaidDiagramKind('const x = 1;')).toBeUndefined();
  });

  it('renders a flowchart with boxed nodes and arrows', () => {
    const rendered = renderMermaidToLines(
      `flowchart TD
  Start[User Prompt] -->|parse| Engine(NIM Router)
  Engine --> End[Reply]`,
      80,
    );
    expect(rendered).not.toBeNull();
    const joined = rendered!.lines.join('\n');
    expect(joined).toContain('User Prompt');
    expect(joined).toContain('NIM Router');
    expect(joined).toContain('Reply');
    expect(joined).toContain('▼');
  });

  it('renders LR flowcharts horizontally when space allows and folds on narrow width', () => {
    const lr = 'flowchart LR\n  A[One] --> B[Two]';
    const wide = renderMermaidToLines(lr, 80);
    const narrow = renderMermaidToLines(lr, 24);
    expect(wide!.lines.join('\n')).toContain('▶');
    expect(narrow!.lines.join('\n')).toContain('▼');
  });

  it('renders sequence diagrams with participant lifelines and messages', () => {
    const rendered = renderMermaidToLines(
      `sequenceDiagram
  User->>Agent: Ask question
  Agent-->>User: Stream answer`,
      80,
    );
    expect(rendered).not.toBeNull();
    const joined = rendered!.lines.join('\n');
    expect(joined).toContain('User');
    expect(joined).toContain('Agent');
    expect(joined).toContain('Ask question');
    expect(joined).toContain('Stream answer');
  });

  it('renders state, timeline, gitGraph, and gantt diagrams cleanly', () => {
    expect(
      renderMermaidToLines(
        'stateDiagram-v2\n  [*] --> Idle\n  Idle --> Done',
        80,
      )!.lines.join('\n'),
    ).toContain('Idle');
    expect(
      renderMermaidToLines(
        'timeline\n  title Roadmap\n  Q1 : Launch',
        80,
      )!.lines.join('\n'),
    ).toContain('Launch');
    expect(
      renderMermaidToLines(
        'gitGraph\n  commit id: "init"\n  branch feat\n  commit id: "ui"',
        80,
      )!.lines.join('\n'),
    ).toContain('branch feat');
    expect(
      renderMermaidToLines(
        'gantt\n  title Build\n  section Core\n  Lexer :a, 2d',
        80,
      )!.lines.join('\n'),
    ).toContain('Lexer');
  });
});
