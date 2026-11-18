import React, { useState } from 'react';
import {
  Folder,
  ShieldCheck,
  ShieldAlert,
  Sliders,
  Sidebar as SidebarIcon,
  Cpu,
} from 'lucide-react';
import { useApp } from '../../stores/AppContext';

interface WorkspaceHeaderProps {
  isSidebarOpen: boolean;
  onToggleSidebar: () => void;
  onOpenSettings: () => void;
}

export const WorkspaceHeader: React.FC<WorkspaceHeaderProps> = ({
  isSidebarOpen,
  onToggleSidebar,
  onOpenSettings,
}) => {
  const {
    workspaceInfo,
    availableModels,
    activeModel,
    switchModel,
    authorizeCurrentWorkspace,
    runtimeStatus,
    isRuntimeReady,
    runningTurnId,
    isSubmittingTurn,
  } = useApp();

  const [workspaceError, setWorkspaceError] = useState<string | null>(null);
  const [modelError, setModelError] = useState<string | null>(null);
  const isTurnBusy = Boolean(runningTurnId) || isSubmittingTurn;

  const handleModelChange = async (e: React.ChangeEvent<HTMLSelectElement>) => {
    try {
      await switchModel(e.target.value);
      setModelError(null);
    } catch (err) {
      const message = String(err);
      setModelError(message);
      console.error('Failed to switch model:', err);
    }
  };

  const getWorkspaceBasename = (p?: string) => {
    if (!p) return 'No Workspace';
    const parts = p.replace(/[\\/]+$/, '').split(/[\\/]/);
    return parts[parts.length - 1] || p;
  };

  return (
    <header
      style={{
        height: '42px',
        borderBottom: '1px solid var(--border-primary)',
        backgroundColor: 'var(--bg-secondary)',
        display: 'flex',
        alignItems: 'center',
        padding: '0 12px',
        gap: '12px',
        userSelect: 'none',
      }}
    >
      {/* Sidebar toggle */}
      <button
        type="button"
        onClick={onToggleSidebar}
        className="btn btn-sm"
        style={{ padding: '4px 6px', background: 'transparent', border: 'none' }}
        title={isSidebarOpen ? 'Close sidebar' : 'Open sidebar'}
      >
        <SidebarIcon size={14} color="var(--text-secondary)" />
      </button>

      {/* Workspace Path & Trust Indicator */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          fontSize: '12px',
        }}
      >
        <Folder size={14} color="var(--text-secondary)" />
        <span
          title={workspaceInfo?.path}
          style={{
            fontFamily: 'var(--font-mono)',
            fontWeight: 600,
            color: 'var(--text-primary)',
            maxWidth: '300px',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {getWorkspaceBasename(workspaceInfo?.path)}
        </span>

        {workspaceInfo && (
          workspaceInfo.is_trusted ? (
            <span
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: '4px',
                fontSize: '11px',
                color: 'var(--accent-green)',
                backgroundColor: 'rgba(35, 134, 54, 0.1)',
                padding: '2px 6px',
                borderRadius: 'var(--radius-sm)',
              }}
              title="Workspace is trusted. Full execution enabled."
            >
              <ShieldCheck size={11} /> Trusted
            </span>
          ) : (
            <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
              <span
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: '4px',
                  fontSize: '11px',
                  color: 'var(--accent-red)',
                  backgroundColor: 'rgba(218, 54, 51, 0.1)',
                  padding: '2px 6px',
                  borderRadius: 'var(--radius-sm)',
                }}
                title="Workspace is untrusted. Execution is guarded."
              >
                <ShieldAlert size={11} /> Untrusted
              </span>
              <button
                type="button"
                onClick={() => {
                  setWorkspaceError(null);
                  void authorizeCurrentWorkspace().catch((err) => {
                    setWorkspaceError(String(err));
                  });
                }}
                className="btn btn-sm"
                style={{ padding: '2px 6px', fontSize: '10px' }}
              >
                Authorize
              </button>
            </div>
          )
        )}
      </div>

      {/* Spacer */}
      <div style={{ marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: '10px' }}>
        {/* Model Selector */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '11px' }}>
          <Cpu size={13} color="var(--text-secondary)" />
          <select
            value={activeModel}
            onChange={handleModelChange}
            aria-label="Active model"
            disabled={!isRuntimeReady || isTurnBusy || availableModels.length === 0}
            style={{
              padding: '3px 8px',
              borderRadius: 'var(--radius-sm)',
              border: '1px solid var(--border-primary)',
              backgroundColor: 'var(--bg-primary)',
              color: 'var(--text-primary)',
              fontSize: '11px',
              cursor: 'pointer',
              outline: 'none',
            }}
          >
            {availableModels.map((m) => (
              <option key={m.id} value={m.id}>
                {m.name} ({m.originator})
              </option>
            ))}
          </select>
        </div>

        {/* Runtime Status Dot */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '6px',
            fontSize: '11px',
            color: 'var(--text-secondary)',
          }}
          title={`Runtime: ${runtimeStatus?.state || 'Unknown'}`}
        >
          <span
            style={{
              display: 'inline-block',
              width: '8px',
              height: '8px',
              borderRadius: '50%',
              backgroundColor: isRuntimeReady
                ? 'var(--accent-green)'
                : runtimeStatus?.state === 'Initializing'
                ? 'var(--accent-orange)'
                : 'var(--accent-red)',
            }}
          />
          <span>{runtimeStatus?.state || 'Offline'}</span>
        </div>

        {/* Settings button */}
        <button
          type="button"
          onClick={onOpenSettings}
          className="btn btn-sm"
          style={{ padding: '4px 6px', background: 'transparent', border: 'none' }}
          title="Runtime Settings"
        >
          <Sliders size={14} color="var(--text-secondary)" />
        </button>
        {(workspaceError || modelError) && (
          <div
            role="alert"
            style={{
              position: 'absolute',
              top: '46px',
              right: '12px',
              zIndex: 20,
              maxWidth: '420px',
              padding: '8px 12px',
              borderRadius: 'var(--radius-sm)',
              border: '1px solid var(--accent-red)',
              background: 'var(--bg-secondary)',
              color: 'var(--accent-red)',
              fontSize: '11px',
            }}
          >
            {workspaceError || modelError}
          </div>
        )}
      </div>
    </header>
  );
};
