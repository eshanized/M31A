import React from 'react';
import { Plus, MessageSquare, Clock, Loader2 } from 'lucide-react';
import { useApp } from '../../stores/AppContext';

export const SessionSidebar: React.FC<{ isOpen: boolean }> = ({ isOpen }) => {
  const {
    sessions,
    activeSessionId,
    selectSession,
    createNewSession,
    isLoadingSessions,
    isRuntimeReady,
  } = useApp();

  if (!isOpen) return null;

  return (
    <aside
      style={{
        width: '240px',
        borderRight: '1px solid var(--border-primary)',
        backgroundColor: 'var(--bg-secondary)',
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        userSelect: 'none',
      }}
    >
      {/* Header & New Session Button */}
      <div
        style={{
          padding: '12px',
          borderBottom: '1px solid var(--border-primary)',
          display: 'flex',
          flexDirection: 'column',
          gap: '8px',
        }}
      >
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
          }}
        >
          <span style={{ fontSize: '11px', fontWeight: 600, color: 'var(--text-secondary)', textTransform: 'uppercase' }}>
            Sessions
          </span>
          {isLoadingSessions && <Loader2 size={12} className="spin" />}
        </div>

        <button
          type="button"
          onClick={() => createNewSession()}
          disabled={!isRuntimeReady}
          className="btn btn-sm"
          style={{ width: '100%', justifyContent: 'center' }}
        >
          <Plus size={13} /> New Session
        </button>
      </div>

      {/* Session List */}
      <div
        style={{
          flex: 1,
          overflowY: 'auto',
          padding: '6px',
          display: 'flex',
          flexDirection: 'column',
          gap: '2px',
        }}
      >
        {sessions.length === 0 ? (
          <div
            style={{
              padding: '24px 12px',
              textAlign: 'center',
              color: 'var(--text-muted)',
              fontSize: '12px',
            }}
          >
            No active sessions
          </div>
        ) : (
          sessions.map((sess) => {
            const isActive = sess.id === activeSessionId;
            return (
              <div
                key={sess.id}
                onClick={() => selectSession(sess.id)}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px',
                  padding: '8px 10px',
                  borderRadius: 'var(--radius-sm)',
                  backgroundColor: isActive ? 'var(--bg-tertiary)' : 'transparent',
                  border: `1px solid ${isActive ? 'var(--border-primary)' : 'transparent'}`,
                  cursor: 'pointer',
                  color: isActive ? 'var(--text-primary)' : 'var(--text-secondary)',
                  fontSize: '12px',
                  transition: 'all 0.15s ease',
                }}
              >
                <MessageSquare
                  size={13}
                  color={isActive ? 'var(--accent-blue)' : 'var(--text-muted)'}
                />
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div
                    style={{
                      whiteSpace: 'nowrap',
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      fontWeight: isActive ? 600 : 400,
                    }}
                  >
                    {sess.title || sess.id.substring(0, 12)}
                  </div>
                  <div
                    style={{
                      fontSize: '10px',
                      color: 'var(--text-muted)',
                      display: 'flex',
                      alignItems: 'center',
                      gap: '4px',
                    }}
                  >
                    <Clock size={10} />
                    {new Date(sess.updated_at * 1000).toLocaleDateString([], {
                      month: 'short',
                      day: 'numeric',
                    })}
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>
    </aside>
  );
};
