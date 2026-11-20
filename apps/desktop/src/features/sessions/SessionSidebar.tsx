import React, { useState } from 'react';
import {
  Plus,
  MessageSquare,
  Clock,
  Loader2,
  FolderTree,
  Search,
  Edit2,
  GitFork,
  Download,
  Trash2,
} from 'lucide-react';
import { useApp } from '../../stores/AppContext';
import { WorkspaceExplorer } from '../workspace/WorkspaceExplorer';

export const SessionSidebar: React.FC<{ isOpen: boolean }> = ({ isOpen }) => {
  const {
    sessions,
    activeSessionId,
    selectSession,
    createNewSession,
    renameSession,
    deleteSession,
    forkSession,
    exportSession,
    isLoadingSessions,
    isRuntimeReady,
    activeSidebarTab,
    setActiveSidebarTab,
  } = useApp();

  const [searchFilter, setSearchFilter] = useState('');
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState('');

  if (!isOpen) return null;

  const handleStartRename = (e: React.MouseEvent, id: string, currentTitle: string) => {
    e.stopPropagation();
    setEditingSessionId(id);
    setEditingTitle(currentTitle);
  };

  const handleSaveRename = async (id: string) => {
    if (editingTitle.trim()) {
      await renameSession(id, editingTitle.trim());
    }
    setEditingSessionId(null);
  };

  const handleKeyDownRename = (e: React.KeyboardEvent, id: string) => {
    if (e.key === 'Enter') {
      void handleSaveRename(id);
    } else if (e.key === 'Escape') {
      setEditingSessionId(null);
    }
  };

  const handleDeleteSession = async (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    if (window.confirm('Are you sure you want to delete this session permanently?')) {
      await deleteSession(id);
    }
  };

  const handleForkSession = async (e: React.MouseEvent, id: string, title: string) => {
    e.stopPropagation();
    await forkSession(id, `${title} (Fork)`);
  };

  const handleExportSession = async (e: React.MouseEvent, id: string, title: string) => {
    e.stopPropagation();
    try {
      const markdown = await exportSession(id);
      const blob = new Blob([markdown], { type: 'text/markdown' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `session-${title.replace(/[^a-zA-Z0-9_-]/g, '_')}.md`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      console.error('Failed to export session:', err);
    }
  };

  const filteredSessions = sessions.filter((s) =>
    (s.title || s.id).toLowerCase().includes(searchFilter.toLowerCase())
  );

  return (
    <aside
      style={{
        width: '260px',
        borderRight: '1px solid var(--border-primary)',
        backgroundColor: 'var(--bg-secondary)',
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        userSelect: 'none',
      }}
    >
      {/* Top Navigation Tabs */}
      <div
        style={{
          display: 'flex',
          borderBottom: '1px solid var(--border-primary)',
          backgroundColor: 'var(--bg-primary)',
        }}
      >
        <button
          type="button"
          onClick={() => setActiveSidebarTab('sessions')}
          className="btn"
          style={{
            flex: 1,
            borderRadius: 0,
            border: 'none',
            borderBottom: activeSidebarTab === 'sessions' ? '2px solid var(--accent-blue)' : '2px solid transparent',
            backgroundColor: activeSidebarTab === 'sessions' ? 'var(--bg-secondary)' : 'transparent',
            color: activeSidebarTab === 'sessions' ? 'var(--text-primary)' : 'var(--text-secondary)',
            padding: '8px 12px',
            fontSize: '11px',
            fontWeight: 600,
            justifyContent: 'center',
            gap: '6px',
          }}
        >
          <MessageSquare size={13} /> Sessions
        </button>
        <button
          type="button"
          onClick={() => setActiveSidebarTab('files')}
          className="btn"
          style={{
            flex: 1,
            borderRadius: 0,
            border: 'none',
            borderBottom: activeSidebarTab === 'files' ? '2px solid var(--accent-blue)' : '2px solid transparent',
            backgroundColor: activeSidebarTab === 'files' ? 'var(--bg-secondary)' : 'transparent',
            color: activeSidebarTab === 'files' ? 'var(--text-primary)' : 'var(--text-secondary)',
            padding: '8px 12px',
            fontSize: '11px',
            fontWeight: 600,
            justifyContent: 'center',
            gap: '6px',
          }}
        >
          <FolderTree size={13} /> Files
        </button>
      </div>

      {/* Files Tab Content */}
      {activeSidebarTab === 'files' ? (
        <div style={{ flex: 1, overflow: 'hidden' }}>
          <WorkspaceExplorer />
        </div>
      ) : (
        /* Sessions Tab Content */
        <div style={{ display: 'flex', flexDirection: 'column', flex: 1, overflow: 'hidden' }}>
          {/* Header & Controls */}
          <div
            style={{
              padding: '10px 12px',
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
                Threads ({sessions.length})
              </span>
              {isLoadingSessions && <Loader2 size={12} className="spin" />}
            </div>

            <button
              type="button"
              onClick={() => void createNewSession()}
              disabled={!isRuntimeReady}
              className="btn btn-sm btn-primary"
              style={{ width: '100%', justifyContent: 'center' }}
            >
              <Plus size={13} /> New Session
            </button>

            {/* Filter Search */}
            <div style={{ position: 'relative' }}>
              <Search
                size={12}
                color="var(--text-muted)"
                style={{ position: 'absolute', left: '8px', top: '7px' }}
              />
              <input
                type="text"
                placeholder="Search sessions..."
                value={searchFilter}
                onChange={(e) => setSearchFilter(e.target.value)}
                style={{
                  width: '100%',
                  padding: '4px 8px 4px 26px',
                  fontSize: '11px',
                  borderRadius: 'var(--radius-sm)',
                  border: '1px solid var(--border-primary)',
                  backgroundColor: 'var(--bg-primary)',
                  color: 'var(--text-primary)',
                  outline: 'none',
                }}
              />
            </div>
          </div>

          {/* Sessions List */}
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
            {filteredSessions.length === 0 ? (
              <div
                style={{
                  padding: '24px 12px',
                  textAlign: 'center',
                  color: 'var(--text-muted)',
                  fontSize: '12px',
                }}
              >
                {searchFilter ? 'No matching sessions' : 'No active sessions'}
              </div>
            ) : (
              filteredSessions.map((sess) => {
                const isActive = sess.id === activeSessionId;
                const isEditing = editingSessionId === sess.id;

                return (
                  <div
                    key={sess.id}
                    onClick={() => void selectSession(sess.id)}
                    style={{
                      display: 'flex',
                      flexDirection: 'column',
                      padding: '8px 10px',
                      borderRadius: 'var(--radius-sm)',
                      backgroundColor: isActive ? 'var(--bg-tertiary)' : 'transparent',
                      border: `1px solid ${isActive ? 'var(--border-primary)' : 'transparent'}`,
                      cursor: 'pointer',
                      color: isActive ? 'var(--text-primary)' : 'var(--text-secondary)',
                      fontSize: '12px',
                      transition: 'all 0.15s ease',
                      position: 'relative',
                      gap: '4px',
                    }}
                  >
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <MessageSquare
                        size={13}
                        color={isActive ? 'var(--accent-blue)' : 'var(--text-muted)'}
                      />
                      <div style={{ flex: 1, minWidth: 0 }}>
                        {isEditing ? (
                          <input
                            type="text"
                            value={editingTitle}
                            autoFocus
                            onClick={(e) => e.stopPropagation()}
                            onChange={(e) => setEditingTitle(e.target.value)}
                            onBlur={() => void handleSaveRename(sess.id)}
                            onKeyDown={(e) => handleKeyDownRename(e, sess.id)}
                            style={{
                              width: '100%',
                              padding: '2px 4px',
                              fontSize: '11px',
                              borderRadius: '2px',
                              border: '1px solid var(--accent-blue)',
                              backgroundColor: 'var(--bg-primary)',
                              color: 'var(--text-primary)',
                              outline: 'none',
                            }}
                          />
                        ) : (
                          <div
                            style={{
                              whiteSpace: 'nowrap',
                              overflow: 'hidden',
                              textOverflow: 'ellipsis',
                              fontWeight: isActive ? 600 : 400,
                            }}
                            title={sess.title || sess.id}
                          >
                            {sess.title || sess.id.substring(0, 12)}
                          </div>
                        )}
                      </div>
                    </div>

                    {/* Metadata & Actions row */}
                    <div
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'space-between',
                        fontSize: '10px',
                        color: 'var(--text-muted)',
                        paddingLeft: '21px',
                      }}
                    >
                      <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                        <Clock size={10} />
                        {new Date(sess.updated_at * 1000).toLocaleDateString([], {
                          month: 'short',
                          day: 'numeric',
                        })}
                      </div>

                      {/* Action buttons */}
                      <div
                        style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: '2px',
                          opacity: isActive ? 1 : 0.7,
                        }}
                      >
                        <button
                          type="button"
                          onClick={(e) => handleStartRename(e, sess.id, sess.title || '')}
                          className="btn btn-sm"
                          style={{ padding: '2px', background: 'transparent', border: 'none' }}
                          title="Rename title"
                        >
                          <Edit2 size={11} color="var(--text-secondary)" />
                        </button>
                        <button
                          type="button"
                          onClick={(e) => void handleForkSession(e, sess.id, sess.title || 'Session')}
                          className="btn btn-sm"
                          style={{ padding: '2px', background: 'transparent', border: 'none' }}
                          title="Fork session"
                        >
                          <GitFork size={11} color="var(--text-secondary)" />
                        </button>
                        <button
                          type="button"
                          onClick={(e) => void handleExportSession(e, sess.id, sess.title || 'Session')}
                          className="btn btn-sm"
                          style={{ padding: '2px', background: 'transparent', border: 'none' }}
                          title="Export transcript (.md)"
                        >
                          <Download size={11} color="var(--text-secondary)" />
                        </button>
                        <button
                          type="button"
                          onClick={(e) => void handleDeleteSession(e, sess.id)}
                          className="btn btn-sm"
                          style={{ padding: '2px', background: 'transparent', border: 'none' }}
                          title="Delete session"
                        >
                          <Trash2 size={11} color="var(--accent-red)" />
                        </button>
                      </div>
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </div>
      )}
    </aside>
  );
};
