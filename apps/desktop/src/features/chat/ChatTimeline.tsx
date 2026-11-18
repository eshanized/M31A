import React, { useEffect, useRef } from 'react';
import { Terminal, Shield, Cpu } from 'lucide-react';
import { useApp } from '../../stores/AppContext';
import { MessageItem } from './MessageItem';

export const ChatTimeline: React.FC = () => {
  const { messages, activeSessionId, isRuntimeReady, submitTurn } = useApp();
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  if (messages.length === 0) {
    return (
      <div
        style={{
          flex: 1,
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
          padding: '32px',
          color: 'var(--text-secondary)',
          textAlign: 'center',
        }}
      >
        <div
          style={{
            width: '48px',
            height: '48px',
            borderRadius: '12px',
            backgroundColor: 'var(--bg-secondary)',
            border: '1px solid var(--border-primary)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            marginBottom: '16px',
          }}
        >
          <Terminal size={24} color="var(--accent-blue)" />
        </div>

        <h2 style={{ fontSize: '18px', fontWeight: 600, color: 'var(--text-primary)', marginBottom: '8px' }}>
          M31A Autonomous Engineering Runtime
        </h2>

        <p style={{ maxWidth: '520px', fontSize: '13px', lineHeight: 1.5, marginBottom: '24px' }}>
          An authoritative desktop environment backed by the Rust execution engine.
          Commands, tool invocations, and workspace modifications are governed by strict security policies.
        </p>

        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
            gap: '12px',
            maxWidth: '680px',
            width: '100%',
          }}
        >
          <button
            type="button"
            onClick={() => submitTurn('Review the current repository structure and summarize workspace status.')}
            disabled={!isRuntimeReady}
            className="btn"
            style={{
              padding: '12px',
              textAlign: 'left',
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'flex-start',
              gap: '4px',
              height: 'auto',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontWeight: 600 }}>
              <Shield size={14} color="var(--accent-green)" /> Workspace Inspection
            </div>
            <div style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
              Inspect repo structure and check active branch/status.
            </div>
          </button>

          <button
            type="button"
            onClick={() => submitTurn('Run verification checks on the workspace.')}
            disabled={!isRuntimeReady}
            className="btn"
            style={{
              padding: '12px',
              textAlign: 'left',
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'flex-start',
              gap: '4px',
              height: 'auto',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontWeight: 600 }}>
              <Cpu size={14} color="var(--accent-blue)" /> Verify Workspace
            </div>
            <div style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
              Run cargo check, tests, or linters in the active workspace.
            </div>
          </button>
        </div>

        {activeSessionId && (
          <div style={{ marginTop: '20px', fontSize: '11px', color: 'var(--text-muted)' }}>
            Session ID: <code style={{ color: 'var(--text-secondary)' }}>{activeSessionId}</code>
          </div>
        )}
      </div>
    );
  }

  return (
    <div
      style={{
        flex: 1,
        overflowY: 'auto',
        padding: '16px 24px',
      }}
    >
      {messages.map((msg) => (
        <MessageItem key={msg.id} message={msg} />
      ))}
      <div ref={bottomRef} />
    </div>
  );
};
