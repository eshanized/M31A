import React from 'react';
import { X, Server, Layers, Cpu, ShieldCheck } from 'lucide-react';
import { useApp } from '../../stores/AppContext';

export const SettingsModal: React.FC<{ isOpen: boolean; onClose: () => void }> = ({
  isOpen,
  onClose,
}) => {
  const { environmentInfo, runtimeStatus, workspaceInfo, activeModel, availableModels } =
    useApp();

  if (!isOpen) return null;

  const currentModelInfo = availableModels.find((m) => m.id === activeModel);

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        backgroundColor: 'var(--bg-overlay)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
      }}
    >
      <div
        style={{
          width: '520px',
          maxWidth: '90vw',
          backgroundColor: 'var(--bg-secondary)',
          border: '1px solid var(--border-primary)',
          borderRadius: 'var(--radius-lg)',
          boxShadow: '0 8px 24px rgba(0, 0, 0, 0.4)',
          overflow: 'hidden',
          display: 'flex',
          flexDirection: 'column',
        }}
      >
        {/* Header */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            padding: '12px 16px',
            borderBottom: '1px solid var(--border-primary)',
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', fontWeight: 600 }}>
            <Server size={16} color="var(--accent-blue)" />
            <span>Runtime Information & Settings</span>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="btn btn-sm"
            style={{ background: 'transparent', border: 'none', padding: '4px' }}
          >
            <X size={16} />
          </button>
        </div>

        {/* Content */}
        <div style={{ padding: '16px', display: 'flex', flexDirection: 'column', gap: '16px' }}>
          {/* Environment */}
          <div>
            <div
              style={{
                fontSize: '11px',
                fontWeight: 600,
                color: 'var(--text-secondary)',
                textTransform: 'uppercase',
                marginBottom: '8px',
                display: 'flex',
                alignItems: 'center',
                gap: '6px',
              }}
            >
              <Layers size={13} /> Environment
            </div>
            <div
              style={{
                backgroundColor: 'var(--bg-primary)',
                border: '1px solid var(--border-muted)',
                borderRadius: 'var(--radius-sm)',
                padding: '10px 12px',
                display: 'grid',
                gridTemplateColumns: '120px 1fr',
                rowGap: '6px',
                fontSize: '12px',
              }}
            >
              <span style={{ color: 'var(--text-secondary)' }}>M31A Version:</span>
              <span style={{ fontFamily: 'var(--font-mono)' }}>{environmentInfo?.app_version || '0.1.15'}</span>

              <span style={{ color: 'var(--text-secondary)' }}>Platform:</span>
              <span style={{ fontFamily: 'var(--font-mono)' }}>
                {environmentInfo?.target_platform} ({environmentInfo?.target_arch})
              </span>

              <span style={{ color: 'var(--text-secondary)' }}>Runtime State:</span>
              <span style={{ fontFamily: 'var(--font-mono)' }}>{runtimeStatus?.state || 'Unknown'}</span>
            </div>
          </div>

          {/* Active Model */}
          <div>
            <div
              style={{
                fontSize: '11px',
                fontWeight: 600,
                color: 'var(--text-secondary)',
                textTransform: 'uppercase',
                marginBottom: '8px',
                display: 'flex',
                alignItems: 'center',
                gap: '6px',
              }}
            >
              <Cpu size={13} /> Active Provider & Model
            </div>
            <div
              style={{
                backgroundColor: 'var(--bg-primary)',
                border: '1px solid var(--border-muted)',
                borderRadius: 'var(--radius-sm)',
                padding: '10px 12px',
                display: 'grid',
                gridTemplateColumns: '120px 1fr',
                rowGap: '6px',
                fontSize: '12px',
              }}
            >
              <span style={{ color: 'var(--text-secondary)' }}>Model ID:</span>
              <span style={{ fontFamily: 'var(--font-mono)' }}>{currentModelInfo?.id || activeModel}</span>

              <span style={{ color: 'var(--text-secondary)' }}>Provider:</span>
              <span>{currentModelInfo?.originator || 'Configured Provider'}</span>

              <span style={{ color: 'var(--text-secondary)' }}>Context Window:</span>
              <span style={{ fontFamily: 'var(--font-mono)' }}>
                {currentModelInfo ? `${currentModelInfo.context_window.toLocaleString()} tokens` : 'N/A'}
              </span>

              <span style={{ color: 'var(--text-secondary)' }}>Capabilities:</span>
              <span>
                {currentModelInfo?.supports_tools ? 'Tools • ' : ''}
                {currentModelInfo?.supports_thinking ? 'Thinking • ' : ''}
                {currentModelInfo?.supports_vision ? 'Vision' : ''}
              </span>
            </div>
          </div>

          {/* Workspace Security */}
          <div>
            <div
              style={{
                fontSize: '11px',
                fontWeight: 600,
                color: 'var(--text-secondary)',
                textTransform: 'uppercase',
                marginBottom: '8px',
                display: 'flex',
                alignItems: 'center',
                gap: '6px',
              }}
            >
              <ShieldCheck size={13} /> Workspace Security
            </div>
            <div
              style={{
                backgroundColor: 'var(--bg-primary)',
                border: '1px solid var(--border-muted)',
                borderRadius: 'var(--radius-sm)',
                padding: '10px 12px',
                fontSize: '12px',
              }}
            >
              <div style={{ color: 'var(--text-secondary)', marginBottom: '4px' }}>Path:</div>
              <div
                style={{
                  fontFamily: 'var(--font-mono)',
                  fontSize: '11px',
                  backgroundColor: 'var(--bg-tertiary)',
                  padding: '6px 8px',
                  borderRadius: 'var(--radius-sm)',
                  wordBreak: 'break-all',
                  marginBottom: '6px',
                }}
              >
                {workspaceInfo?.path}
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                <span style={{ color: 'var(--text-secondary)' }}>Trust Status:</span>
                <span
                  style={{
                    color: workspaceInfo?.is_trusted ? 'var(--accent-green)' : 'var(--accent-red)',
                    fontWeight: 600,
                  }}
                >
                  {workspaceInfo?.is_trusted ? 'Authoritative Trust Granted' : 'Untrusted (Guarded Mode)'}
                </span>
              </div>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div
          style={{
            display: 'flex',
            justifyContent: 'flex-end',
            padding: '12px 16px',
            borderTop: '1px solid var(--border-primary)',
          }}
        >
          <button type="button" onClick={onClose} className="btn btn-sm">
            Close
          </button>
        </div>
      </div>
    </div>
  );
};
