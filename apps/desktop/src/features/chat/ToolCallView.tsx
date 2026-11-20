import React, { useState } from 'react';
import { ChevronDown, ChevronRight, CheckCircle2, XCircle, Loader2, Wrench } from 'lucide-react';
import type { ToolCallItem } from '../../stores/AppContext';

export const ToolCallView: React.FC<{ toolCall: ToolCallItem }> = ({ toolCall }) => {
  const [isExpanded, setIsExpanded] = useState(false);

  const formatJsonString = (raw?: string) => {
    if (!raw) return '';
    try {
      const parsed = JSON.parse(raw);
      return JSON.stringify(parsed, null, 2);
    } catch {
      return raw;
    }
  };

  return (
    <div
      style={{
        margin: '6px 0',
        borderRadius: 'var(--radius-md)',
        border: '1px solid var(--border-primary)',
        backgroundColor: 'var(--bg-secondary)',
        fontSize: '12px',
        overflow: 'hidden',
      }}
    >
      <button
        type="button"
        onClick={() => setIsExpanded((expanded) => !expanded)}
        aria-expanded={isExpanded}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          width: '100%',
          padding: '8px 12px',
          cursor: 'pointer',
          userSelect: 'none',
          textAlign: 'left',
          border: 0,
          background: 'transparent',
          color: 'inherit',
        }}
      >
        <span style={{ color: 'var(--text-secondary)' }}>
          {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </span>

        <Wrench size={14} color="var(--text-secondary)" />

        <span style={{ fontFamily: 'var(--font-mono)', fontWeight: 600, color: '#e6edf3' }}>
          {toolCall.name}
        </span>

        <div style={{ marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: '6px' }}>
          {toolCall.status === 'running' && (
            <span
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: '4px',
                color: 'var(--accent-blue)',
                fontSize: '11px',
              }}
            >
              <Loader2 size={12} className="spin" style={{ animation: 'spin 1s linear infinite' }} />
              Executing
            </span>
          )}
          {toolCall.status === 'success' && (
            <span
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: '4px',
                color: 'var(--accent-green)',
                fontSize: '11px',
              }}
            >
              <CheckCircle2 size={12} />
              Completed
            </span>
          )}
          {toolCall.status === 'failed' && (
            <span
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: '4px',
                color: 'var(--accent-red)',
                fontSize: '11px',
              }}
            >
              <XCircle size={12} />
              Failed
            </span>
          )}
        </div>
      </button>

      {isExpanded && (
        <div
          style={{
            padding: '8px 12px',
            borderTop: '1px solid var(--border-primary)',
            backgroundColor: '#0a0d12',
            fontFamily: 'var(--font-mono)',
            fontSize: '11px',
          }}
        >
          {toolCall.arguments && (
            <div style={{ marginBottom: toolCall.output ? '8px' : '0' }}>
              <div style={{ color: 'var(--text-secondary)', marginBottom: '4px' }}>Arguments:</div>
              <pre
                style={{
                  padding: '6px',
                  backgroundColor: 'var(--bg-tertiary)',
                  borderRadius: 'var(--radius-sm)',
                  overflowX: 'auto',
                  whiteSpace: 'pre-wrap',
                  wordBreak: 'break-all',
                }}
              >
                {formatJsonString(toolCall.arguments)}
              </pre>
            </div>
          )}

          {toolCall.output && (
            <div>
              <div style={{ color: 'var(--text-secondary)', marginBottom: '4px' }}>Output:</div>
              <pre
                style={{
                  padding: '6px',
                  backgroundColor: 'var(--bg-tertiary)',
                  borderRadius: 'var(--radius-sm)',
                  overflowX: 'auto',
                  whiteSpace: 'pre-wrap',
                  wordBreak: 'break-all',
                  maxHeight: '200px',
                  overflowY: 'auto',
                  color: toolCall.status === 'failed' ? 'var(--accent-red)' : 'var(--text-primary)',
                }}
              >
                {toolCall.output}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
};
