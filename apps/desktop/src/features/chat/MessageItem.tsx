import React, { useState } from 'react';
import { User, Bot, BrainCircuit, ChevronDown, ChevronRight, AlertTriangle } from 'lucide-react';
import type { ChatMessage } from '../../stores/AppContext';
import { ToolCallView } from './ToolCallView';
import { CodeBlock } from './CodeBlock';

interface MessageItemProps {
  message: ChatMessage;
}

export const MessageItem: React.FC<MessageItemProps> = ({ message }) => {
  const [isReasoningOpen, setIsReasoningOpen] = useState(false);

  // Parse code blocks in message content: ```lang\ncode\n```
  const renderFormattedContent = (text: string) => {
    if (!text) return null;

    const codeBlockRegex = /```([^\n`]*)\n([\s\S]*?)```/g;
    const parts = [];
    let lastIndex = 0;
    let match;

    while ((match = codeBlockRegex.exec(text)) !== null) {
      if (match.index > lastIndex) {
        parts.push(
          <div
            key={`text-${lastIndex}`}
            style={{ whiteSpace: 'pre-wrap', marginBottom: '8px' }}
          >
            {text.substring(lastIndex, match.index)}
          </div>
        );
      }
      parts.push(
        <CodeBlock
          key={`code-${match.index}`}
          language={match[1]}
          code={match[2]}
        />
      );
      lastIndex = match.index + match[0].length;
    }

    if (lastIndex < text.length) {
      parts.push(
        <div
          key={`text-${lastIndex}`}
          style={{ whiteSpace: 'pre-wrap', marginBottom: '8px' }}
        >
          {text.substring(lastIndex)}
        </div>
      );
    }

    return parts;
  };

  if (message.role === 'system') {
    return (
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          padding: '8px 12px',
          margin: '8px 0',
          borderRadius: 'var(--radius-md)',
          backgroundColor: 'rgba(218, 54, 51, 0.1)',
          border: '1px solid var(--accent-red)',
          color: 'var(--accent-red-hover)',
          fontSize: '12px',
        }}
      >
        <AlertTriangle size={14} />
        <div>{message.content}</div>
      </div>
    );
  }

  const isUser = message.role === 'user';

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        margin: '12px 0',
        padding: '12px 16px',
        borderRadius: 'var(--radius-md)',
        backgroundColor: isUser ? 'rgba(56, 139, 253, 0.05)' : 'var(--bg-secondary)',
        border: `1px solid ${isUser ? 'rgba(56, 139, 253, 0.2)' : 'var(--border-primary)'}`,
      }}
    >
      {/* Header */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          marginBottom: '8px',
          color: 'var(--text-secondary)',
          fontSize: '11px',
          fontWeight: 600,
        }}
      >
        {isUser ? (
          <>
            <User size={14} color="var(--accent-blue)" />
            <span style={{ color: 'var(--text-primary)' }}>User</span>
          </>
        ) : (
          <>
            <Bot size={14} color="var(--accent-green)" />
            <span style={{ color: 'var(--text-primary)' }}>M31A Assistant</span>
          </>
        )}

        <span style={{ marginLeft: 'auto', fontWeight: 'normal' }}>
          {new Date(message.timestamp).toLocaleTimeString([], {
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit',
          })}
        </span>
      </div>

      {/* Reasoning / Thinking block if present */}
      {message.reasoning && (
        <div
          style={{
            margin: '6px 0 10px 0',
            borderRadius: 'var(--radius-sm)',
            border: '1px solid var(--border-muted)',
            backgroundColor: '#070a0e',
            overflow: 'hidden',
          }}
        >
          <button
            type="button"
            onClick={() => setIsReasoningOpen((open) => !open)}
            aria-expanded={isReasoningOpen}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
              width: '100%',
              padding: '6px 10px',
              cursor: 'pointer',
              color: 'var(--text-secondary)',
              fontSize: '11px',
              fontWeight: 500,
              textAlign: 'left',
              background: 'transparent',
              border: 0,
            }}
          >
            {isReasoningOpen ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
            <BrainCircuit size={13} color="var(--accent-orange)" />
            <span>Thinking Process</span>
          </button>
          {isReasoningOpen && (
            <div
              style={{
                padding: '8px 10px',
                borderTop: '1px solid var(--border-muted)',
                fontSize: '11px',
                fontFamily: 'var(--font-mono)',
                color: 'var(--text-secondary)',
                lineHeight: 1.4,
                whiteSpace: 'pre-wrap',
                maxHeight: '180px',
                overflowY: 'auto',
              }}
            >
              {message.reasoning}
            </div>
          )}
        </div>
      )}

      {/* Tool Executions */}
      {message.toolCalls && message.toolCalls.length > 0 && (
        <div style={{ margin: '4px 0 8px 0' }}>
          {message.toolCalls.map((tc) => (
            <ToolCallView key={tc.id} toolCall={tc} />
          ))}
        </div>
      )}

      {/* Message Content */}
      <div
        style={{
          color: 'var(--text-primary)',
          fontSize: '13px',
          lineHeight: '1.6',
          userSelect: 'text',
        }}
      >
        {renderFormattedContent(message.content)}
      </div>

      {/* Streaming indicator */}
      {message.status === 'streaming' && (
        <div
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: '6px',
            marginTop: '8px',
            fontSize: '11px',
            color: 'var(--accent-blue)',
          }}
        >
          <span
            style={{
              display: 'inline-block',
              width: '6px',
              height: '6px',
              borderRadius: '50%',
              backgroundColor: 'var(--accent-blue)',
              animation: 'pulse 1.5s infinite',
            }}
          />
          Generating response...
        </div>
      )}
    </div>
  );
};
