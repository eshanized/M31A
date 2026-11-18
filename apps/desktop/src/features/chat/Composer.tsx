import React, { useState, useRef, useEffect } from 'react';
import { Send, Square, Sparkles, Compass, ShieldCheck } from 'lucide-react';
import { useApp } from '../../stores/AppContext';

export const Composer: React.FC = () => {
  const {
    submitTurn,
    cancelActiveTurn,
    runningTurnId,
    isSubmittingTurn,
    collaborationMode,
    setCollaborationMode,
    isRuntimeReady,
  } = useApp();

  const [input, setInput] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const isBusy = Boolean(runningTurnId) || isSubmittingTurn;

  // Auto focus input on mount and when turn finishes
  useEffect(() => {
    if (!isBusy && textareaRef.current) {
      textareaRef.current.focus();
    }
  }, [isBusy]);

  const handleSubmit = async () => {
    const trimmed = input.trim();
    if (!trimmed || isBusy || !isRuntimeReady) return;

    setInput('');
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
    }
    await submitTurn(trimmed);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setInput(e.target.value);
    e.target.style.height = 'auto';
    e.target.style.height = `${Math.min(e.target.scrollHeight, 180)}px`;
  };

  return (
    <div
      style={{
        padding: '12px 16px',
        borderTop: '1px solid var(--border-primary)',
        backgroundColor: 'var(--bg-secondary)',
      }}
    >
      {/* Collaboration Mode Bar */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          marginBottom: '8px',
          fontSize: '11px',
        }}
      >
        <span style={{ color: 'var(--text-secondary)', fontWeight: 500 }}>Mode:</span>
        <button
          type="button"
          onClick={() => setCollaborationMode('default')}
          className={`btn btn-sm ${collaborationMode === 'default' ? 'btn-primary' : ''}`}
          style={{ padding: '2px 8px' }}
        >
          <Sparkles size={11} /> Default
        </button>
        <button
          type="button"
          onClick={() => setCollaborationMode('plan')}
          className={`btn btn-sm ${collaborationMode === 'plan' ? 'btn-primary' : ''}`}
          style={{ padding: '2px 8px' }}
        >
          <Compass size={11} /> Plan
        </button>
        <button
          type="button"
          onClick={() => setCollaborationMode('review')}
          className={`btn btn-sm ${collaborationMode === 'review' ? 'btn-primary' : ''}`}
          style={{ padding: '2px 8px' }}
        >
          <ShieldCheck size={11} /> Review
        </button>
      </div>

      {/* Input container */}
      <div
        style={{
          display: 'flex',
          gap: '8px',
          alignItems: 'flex-end',
          borderRadius: 'var(--radius-md)',
          border: '1px solid var(--border-primary)',
          backgroundColor: 'var(--bg-primary)',
          padding: '8px 12px',
        }}
      >
        <textarea
          ref={textareaRef}
          value={input}
          onChange={handleInput}
          onKeyDown={handleKeyDown}
          placeholder={
            !isRuntimeReady
              ? 'Runtime initializing or unavailable...'
              : isBusy
              ? 'Turn in progress... Press stop to cancel'
              : 'Ask M31A to implement, debug, or refactor (Enter to send, Shift+Enter for newline)...'
          }
          disabled={!isRuntimeReady}
          rows={1}
          style={{
            flex: 1,
            border: 'none',
            outline: 'none',
            backgroundColor: 'transparent',
            resize: 'none',
            fontSize: '13px',
            color: 'var(--text-primary)',
            maxHeight: '180px',
            lineHeight: 1.5,
          }}
        />

        {isBusy ? (
          <button
            type="button"
            onClick={cancelActiveTurn}
            className="btn btn-danger btn-sm"
            style={{ padding: '6px 10px', height: '32px' }}
            title="Cancel running turn"
          >
            <Square size={13} fill="currentColor" /> Stop
          </button>
        ) : (
          <button
            type="button"
            onClick={handleSubmit}
            disabled={!input.trim() || !isRuntimeReady}
            className="btn btn-primary btn-sm"
            style={{ padding: '6px 12px', height: '32px' }}
            title="Submit turn"
          >
            <Send size={13} /> Send
          </button>
        )}
      </div>
    </div>
  );
};
