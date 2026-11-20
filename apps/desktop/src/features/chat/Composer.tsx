import React, { useState, useRef, useEffect } from 'react';
import {
  Send,
  Square,
  Sparkles,
  Compass,
  ShieldCheck,
  Navigation,
  CheckCircle,
  GitCompare,
  ShieldAlert,
} from 'lucide-react';
import { useApp } from '../../stores/AppContext';

export const Composer: React.FC = () => {
  const {
    submitTurn,
    cancelActiveTurn,
    steerActiveTurn,
    runningTurnId,
    isSubmittingTurn,
    collaborationMode,
    setCollaborationMode,
    isRuntimeReady,
  } = useApp();

  const [input, setInput] = useState('');
  const [steerInput, setSteerInput] = useState('');
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

    const submitted = await submitTurn(trimmed);
    if (submitted) {
      setInput('');
      if (textareaRef.current) {
        textareaRef.current.style.height = 'auto';
      }
    }
  };

  const handleSteer = async () => {
    const trimmed = steerInput.trim();
    if (!trimmed || !runningTurnId) return;
    await steerActiveTurn(trimmed);
    setSteerInput('');
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.nativeEvent.isComposing || e.keyCode === 229) return;
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

  const applyQuickPrompt = (prompt: string) => {
    setInput(prompt);
    if (textareaRef.current) {
      textareaRef.current.focus();
    }
  };

  return (
    <div
      style={{
        padding: '12px 16px',
        borderTop: '1px solid var(--border-primary)',
        backgroundColor: 'var(--bg-secondary)',
      }}
    >
      {/* Collaboration Mode Bar & Quick Prompts */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          marginBottom: '8px',
          fontSize: '11px',
          gap: '8px',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
          <span style={{ color: 'var(--text-secondary)', fontWeight: 500 }}>Mode:</span>
          <button
            type="button"
            onClick={() => setCollaborationMode('default')}
            className={`btn btn-sm ${collaborationMode === 'default' ? 'btn-primary' : ''}`}
            style={{ padding: '2px 8px' }}
            title="Standard autonomous execution"
          >
            <Sparkles size={11} /> Default
          </button>
          <button
            type="button"
            onClick={() => setCollaborationMode('plan')}
            className={`btn btn-sm ${collaborationMode === 'plan' ? 'btn-primary' : ''}`}
            style={{ padding: '2px 8px' }}
            title="Read-only planning and design mode"
          >
            <Compass size={11} /> Plan
          </button>
          <button
            type="button"
            onClick={() => setCollaborationMode('review')}
            className={`btn btn-sm ${collaborationMode === 'review' ? 'btn-primary' : ''}`}
            style={{ padding: '2px 8px' }}
            title="Audit and verification review mode"
          >
            <ShieldCheck size={11} /> Review
          </button>
        </div>

        {/* Quick Prompt Pills when not busy and input is empty */}
        {!isBusy && !input && (
          <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
            <button
              type="button"
              onClick={() => applyQuickPrompt('Review workspace git status and diff.')}
              className="btn btn-sm"
              style={{ padding: '2px 6px', fontSize: '10px' }}
            >
              <GitCompare size={10} /> Git Diff
            </button>
            <button
              type="button"
              onClick={() => applyQuickPrompt('Verify workspace integrity and run all tests.')}
              className="btn btn-sm"
              style={{ padding: '2px 6px', fontSize: '10px' }}
            >
              <CheckCircle size={10} /> Verify All
            </button>
            <button
              type="button"
              onClick={() => applyQuickPrompt('Perform security audit and sandbox boundary review.')}
              className="btn btn-sm"
              style={{ padding: '2px 6px', fontSize: '10px' }}
            >
              <ShieldAlert size={10} /> Audit
            </button>
          </div>
        )}
      </div>

      {/* When turn is running: Live Steering Bar */}
      {isBusy && runningTurnId ? (
        <div
          style={{
            display: 'flex',
            gap: '8px',
            alignItems: 'center',
            borderRadius: 'var(--radius-md)',
            border: '1px solid var(--accent-blue)',
            backgroundColor: 'var(--bg-primary)',
            padding: '8px 12px',
          }}
        >
          <Navigation size={14} color="var(--accent-blue)" />
          <input
            type="text"
            placeholder="Steer active turn in real time (press Enter to steer)..."
            value={steerInput}
            onChange={(e) => setSteerInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                void handleSteer();
              }
            }}
            style={{
              flex: 1,
              border: 'none',
              outline: 'none',
              backgroundColor: 'transparent',
              fontSize: '12px',
              color: 'var(--text-primary)',
            }}
          />
          <button
            type="button"
            onClick={handleSteer}
            disabled={!steerInput.trim()}
            className="btn btn-primary btn-sm"
            style={{ padding: '4px 10px', height: '28px' }}
            title="Send steering instruction"
          >
            Steer
          </button>
          <button
            type="button"
            onClick={cancelActiveTurn}
            className="btn btn-danger btn-sm"
            style={{ padding: '4px 10px', height: '28px' }}
            title="Cancel running turn"
          >
            <Square size={11} fill="currentColor" /> Stop
          </button>
        </div>
      ) : (
        /* Standard Turn Submission Input */
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
                ? 'Turn in progress...'
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

          <button
            type="button"
            onClick={handleSubmit}
            disabled={!input.trim() || !isRuntimeReady || isBusy}
            className="btn btn-primary btn-sm"
            style={{ padding: '6px 12px', height: '32px' }}
            title="Submit turn"
          >
            <Send size={13} /> Send
          </button>
        </div>
      )}
    </div>
  );
};
