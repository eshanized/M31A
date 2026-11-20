import React, { useEffect, useState } from 'react';
import {
  X,
  Layers,
  Cpu,
  ShieldCheck,
  Wrench,
  Activity,
  FolderOpen,
  CheckCircle2,
  AlertTriangle,
  ChevronDown,
  ChevronRight,
  RefreshCw,
  Search,
} from 'lucide-react';
import { useApp } from '../../stores/AppContext';

export const SettingsModal: React.FC<{ isOpen: boolean; onClose: () => void }> = ({
  isOpen,
  onClose,
}) => {
  const {
    environmentInfo,
    runtimeStatus,
    workspaceInfo,
    activeModel,
    availableModels,
    switchModel,
    authorizeCurrentWorkspace,
    pickWorkspaceFolder,
    registeredTools,
    runtimeConfig,
    fetchRegisteredTools,
    fetchRuntimeConfig,
    refreshRuntime,
    activeSessionId,
    messages,
    pendingApprovals,
    pendingQuestions,
  } = useApp();

  const [activeTab, setActiveTab] = useState<'general' | 'model' | 'security' | 'tools' | 'audit'>('general');
  const [toolSearch, setToolSearch] = useState('');
  const [expandedToolSchemas, setExpandedToolSchemas] = useState<Record<string, boolean>>({});
  const [isRefreshing, setIsRefreshing] = useState(false);

  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  useEffect(() => {
    if (isOpen) {
      void fetchRegisteredTools();
      void fetchRuntimeConfig();
    }
  }, [isOpen, fetchRegisteredTools, fetchRuntimeConfig]);

  if (!isOpen) return null;

  const currentModelInfo = availableModels.find((m) => m.id === activeModel);

  const toggleToolSchema = (name: string) => {
    setExpandedToolSchemas((prev) => ({
      ...prev,
      [name]: !prev[name],
    }));
  };

  const handleRefreshDiagnostics = async () => {
    setIsRefreshing(true);
    try {
      await Promise.all([refreshRuntime(), fetchRegisteredTools(), fetchRuntimeConfig()]);
    } finally {
      setIsRefreshing(false);
    }
  };

  const filteredTools = registeredTools.filter(
    (tool) =>
      tool.name.toLowerCase().includes(toolSearch.toLowerCase()) ||
      tool.description.toLowerCase().includes(toolSearch.toLowerCase())
  );

  const formatSchema = (rawSchema?: string | null) => {
    if (!rawSchema) return 'No parameter schema specified';
    try {
      const parsed = JSON.parse(rawSchema);
      return JSON.stringify(parsed, null, 2);
    } catch {
      return rawSchema;
    }
  };

  return (
    <div
      role="presentation"
      style={{
        position: 'fixed',
        inset: 0,
        backgroundColor: 'var(--bg-overlay)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
        backdropFilter: 'blur(2px)',
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Runtime Information and Settings"
        style={{
          width: '680px',
          maxWidth: '92vw',
          maxHeight: '85vh',
          backgroundColor: 'var(--bg-secondary)',
          border: '1px solid var(--border-primary)',
          borderRadius: 'var(--radius-lg)',
          boxShadow: '0 12px 32px rgba(0, 0, 0, 0.5)',
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
            padding: '14px 18px',
            borderBottom: '1px solid var(--border-primary)',
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', fontWeight: 600, fontSize: '14px' }}>
            <Activity size={18} color="var(--accent-blue)" />
            <span>M31A Runtime Settings & Architecture</span>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="btn btn-sm"
            aria-label="Close settings"
            style={{ background: 'transparent', border: 'none', padding: '4px' }}
          >
            <X size={16} />
          </button>
        </div>

        {/* Tab Navigation */}
        <div
          role="tablist"
          style={{
            display: 'flex',
            borderBottom: '1px solid var(--border-primary)',
            backgroundColor: 'var(--bg-primary)',
            padding: '0 12px',
            gap: '4px',
            overflowX: 'auto',
          }}
        >
          {[
            { id: 'general', label: 'General', icon: Layers },
            { id: 'model', label: 'Model & Engine', icon: Cpu },
            { id: 'security', label: 'Security & Policies', icon: ShieldCheck },
            { id: 'tools', label: `Tool Catalog (${registeredTools.length})`, icon: Wrench },
            { id: 'audit', label: 'Diagnostics', icon: Activity },
          ].map((tab) => {
            const Icon = tab.icon;
            const isSelected = activeTab === tab.id;
            return (
              <button
                key={tab.id}
                type="button"
                role="tab"
                aria-selected={isSelected}
                onClick={() => setActiveTab(tab.id as typeof activeTab)}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '6px',
                  padding: '10px 14px',
                  fontSize: '12px',
                  fontWeight: isSelected ? 600 : 500,
                  color: isSelected ? 'var(--text-primary)' : 'var(--text-secondary)',
                  border: 'none',
                  borderBottom: `2px solid ${isSelected ? 'var(--accent-blue)' : 'transparent'}`,
                  backgroundColor: 'transparent',
                  cursor: 'pointer',
                  transition: 'all 0.15s ease',
                  whiteSpace: 'nowrap',
                }}
              >
                <Icon size={14} color={isSelected ? 'var(--accent-blue)' : 'var(--text-muted)'} />
                <span>{tab.label}</span>
              </button>
            );
          })}
        </div>

        {/* Content Body */}
        <div
          style={{
            padding: '18px 20px',
            overflowY: 'auto',
            flex: 1,
            display: 'flex',
            flexDirection: 'column',
            gap: '16px',
          }}
        >
          {/* TAB 1: General */}
          {activeTab === 'general' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <div>
                <h4 style={{ fontSize: '13px', fontWeight: 600, color: 'var(--text-primary)', marginBottom: '4px' }}>
                  System Environment
                </h4>
                <p style={{ fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '10px' }}>
                  M31A host environment and runtime compilation profile.
                </p>
                <div
                  style={{
                    backgroundColor: 'var(--bg-primary)',
                    border: '1px solid var(--border-muted)',
                    borderRadius: 'var(--radius-sm)',
                    padding: '12px',
                    display: 'grid',
                    gridTemplateColumns: '140px 1fr',
                    rowGap: '8px',
                    fontSize: '12px',
                  }}
                >
                  <span style={{ color: 'var(--text-secondary)' }}>M31A Version:</span>
                  <span style={{ fontFamily: 'var(--font-mono)' }}>{environmentInfo?.app_version || '0.1.15'}</span>

                  <span style={{ color: 'var(--text-secondary)' }}>Target Platform:</span>
                  <span style={{ fontFamily: 'var(--font-mono)' }}>{environmentInfo?.target_platform || 'linux'}</span>

                  <span style={{ color: 'var(--text-secondary)' }}>Architecture:</span>
                  <span style={{ fontFamily: 'var(--font-mono)' }}>{environmentInfo?.target_arch || 'x86_64'}</span>

                  <span style={{ color: 'var(--text-secondary)' }}>Runtime State:</span>
                  <span style={{ fontFamily: 'var(--font-mono)', color: 'var(--accent-green)' }}>
                    {runtimeStatus?.state || 'Ready'}
                  </span>
                </div>
              </div>

              <div>
                <h4 style={{ fontSize: '13px', fontWeight: 600, color: 'var(--text-primary)', marginBottom: '4px' }}>
                  Workspace Boundaries
                </h4>
                <p style={{ fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '10px' }}>
                  The active directory root for file discovery, git inspection, and command execution.
                </p>
                <div
                  style={{
                    backgroundColor: 'var(--bg-primary)',
                    border: '1px solid var(--border-muted)',
                    borderRadius: 'var(--radius-sm)',
                    padding: '12px',
                    fontSize: '12px',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '10px',
                  }}
                >
                  <div>
                    <div style={{ color: 'var(--text-secondary)', marginBottom: '4px' }}>Current Directory:</div>
                    <div
                      style={{
                        fontFamily: 'var(--font-mono)',
                        fontSize: '11px',
                        backgroundColor: 'var(--bg-tertiary)',
                        padding: '6px 8px',
                        borderRadius: 'var(--radius-sm)',
                        wordBreak: 'break-all',
                      }}
                    >
                      {workspaceInfo?.path || 'No workspace selected'}
                    </div>
                  </div>

                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                      <span style={{ color: 'var(--text-secondary)' }}>Trust Authority:</span>
                      {workspaceInfo?.is_trusted ? (
                        <span style={{ color: 'var(--accent-green)', fontWeight: 600, display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
                          <CheckCircle2 size={13} /> Authoritative Trust Granted
                        </span>
                      ) : (
                        <span style={{ color: 'var(--accent-red)', fontWeight: 600, display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
                          <AlertTriangle size={13} /> Untrusted (Guarded Mode)
                        </span>
                      )}
                    </div>

                    <div style={{ display: 'flex', gap: '8px' }}>
                      {!workspaceInfo?.is_trusted && (
                        <button
                          type="button"
                          onClick={() => void authorizeCurrentWorkspace()}
                          className="btn"
                          style={{
                            fontSize: '11px',
                            backgroundColor: 'var(--accent-green)',
                            color: '#fff',
                            borderColor: 'var(--accent-green-hover)',
                          }}
                        >
                          Authorize Workspace
                        </button>
                      )}
                      <button
                        type="button"
                        onClick={() => void pickWorkspaceFolder()}
                        className="btn"
                        style={{ fontSize: '11px' }}
                      >
                        <FolderOpen size={12} /> Choose Directory
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* TAB 2: Model & Provider */}
          {activeTab === 'model' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <div>
                <h4 style={{ fontSize: '13px', fontWeight: 600, color: 'var(--text-primary)', marginBottom: '4px' }}>
                  Model Configuration
                </h4>
                <p style={{ fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '10px' }}>
                  Select the active reasoning and code generation model for this session.
                </p>
                <div
                  style={{
                    backgroundColor: 'var(--bg-primary)',
                    border: '1px solid var(--border-muted)',
                    borderRadius: 'var(--radius-sm)',
                    padding: '12px',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '12px',
                  }}
                >
                  <div>
                    <label
                      htmlFor="settings-model-select"
                      style={{ display: 'block', fontSize: '11px', color: 'var(--text-secondary)', marginBottom: '6px' }}
                    >
                      Active Model:
                    </label>
                    <select
                      id="settings-model-select"
                      value={activeModel}
                      onChange={(e) => void switchModel(e.target.value)}
                      style={{
                        width: '100%',
                        padding: '6px 10px',
                        backgroundColor: 'var(--bg-tertiary)',
                        border: '1px solid var(--border-primary)',
                        borderRadius: 'var(--radius-sm)',
                        color: 'var(--text-primary)',
                        fontSize: '12px',
                      }}
                    >
                      {availableModels.map((model) => (
                        <option key={model.id} value={model.id}>
                          {model.name} ({model.id}) — {model.originator}
                        </option>
                      ))}
                    </select>
                  </div>

                  <div
                    style={{
                      display: 'grid',
                      gridTemplateColumns: '140px 1fr',
                      rowGap: '8px',
                      fontSize: '12px',
                      paddingTop: '6px',
                      borderTop: '1px solid var(--border-muted)',
                    }}
                  >
                    <span style={{ color: 'var(--text-secondary)' }}>Provider / Origin:</span>
                    <span>{currentModelInfo?.originator || 'Configured Upstream'}</span>

                    <span style={{ color: 'var(--text-secondary)' }}>Context Window:</span>
                    <span style={{ fontFamily: 'var(--font-mono)' }}>
                      {currentModelInfo ? `${currentModelInfo.context_window.toLocaleString()} tokens` : 'Default'}
                    </span>

                    <span style={{ color: 'var(--text-secondary)' }}>Capabilities:</span>
                    <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
                      <span
                        style={{
                          fontSize: '10px',
                          padding: '2px 6px',
                          borderRadius: '10px',
                          backgroundColor: currentModelInfo?.supports_tools ? 'rgba(46, 160, 67, 0.15)' : 'var(--bg-tertiary)',
                          color: currentModelInfo?.supports_tools ? 'var(--accent-green)' : 'var(--text-muted)',
                          border: `1px solid ${currentModelInfo?.supports_tools ? 'rgba(46, 160, 67, 0.3)' : 'var(--border-muted)'}`,
                        }}
                      >
                        Tools {currentModelInfo?.supports_tools ? '✓' : '✗'}
                      </span>
                      <span
                        style={{
                          fontSize: '10px',
                          padding: '2px 6px',
                          borderRadius: '10px',
                          backgroundColor: currentModelInfo?.supports_thinking ? 'rgba(210, 153, 34, 0.15)' : 'var(--bg-tertiary)',
                          color: currentModelInfo?.supports_thinking ? 'var(--accent-orange)' : 'var(--text-muted)',
                          border: `1px solid ${currentModelInfo?.supports_thinking ? 'rgba(210, 153, 34, 0.3)' : 'var(--border-muted)'}`,
                        }}
                      >
                        Thinking {currentModelInfo?.supports_thinking ? '✓' : '✗'}
                      </span>
                      <span
                        style={{
                          fontSize: '10px',
                          padding: '2px 6px',
                          borderRadius: '10px',
                          backgroundColor: currentModelInfo?.supports_vision ? 'rgba(56, 139, 253, 0.15)' : 'var(--bg-tertiary)',
                          color: currentModelInfo?.supports_vision ? 'var(--accent-blue)' : 'var(--text-muted)',
                          border: `1px solid ${currentModelInfo?.supports_vision ? 'rgba(56, 139, 253, 0.3)' : 'var(--border-muted)'}`,
                        }}
                      >
                        Vision {currentModelInfo?.supports_vision ? '✓' : '✗'}
                      </span>
                    </div>

                    <span style={{ color: 'var(--text-secondary)' }}>API Base URL:</span>
                    <span style={{ fontFamily: 'var(--font-mono)', wordBreak: 'break-all' }}>
                      {runtimeConfig?.api_base || 'Default upstream (authoritative endpoint)'}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* TAB 3: Security & Policies */}
          {activeTab === 'security' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <div>
                <h4 style={{ fontSize: '13px', fontWeight: 600, color: 'var(--text-primary)', marginBottom: '4px' }}>
                  Execution Governance & Security Policies
                </h4>
                <p style={{ fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '10px' }}>
                  Policies enforced by the Rust execution engine to ensure safe tool dispatch.
                </p>

                <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                  {/* Sandbox Policy */}
                  <div
                    style={{
                      backgroundColor: 'var(--bg-primary)',
                      border: '1px solid var(--border-muted)',
                      borderRadius: 'var(--radius-sm)',
                      padding: '12px',
                    }}
                  >
                    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '6px' }}>
                      <span style={{ fontWeight: 600, fontSize: '12px', color: 'var(--text-primary)' }}>
                        Sandbox Policy
                      </span>
                      <span
                        style={{
                          fontFamily: 'var(--font-mono)',
                          fontSize: '11px',
                          padding: '2px 6px',
                          borderRadius: 'var(--radius-sm)',
                          backgroundColor: 'rgba(56, 139, 253, 0.15)',
                          color: 'var(--accent-blue)',
                          border: '1px solid rgba(56, 139, 253, 0.3)',
                        }}
                      >
                        {runtimeConfig?.sandbox_policy || 'workspace-write'}
                      </span>
                    </div>
                    <p style={{ fontSize: '11px', color: 'var(--text-secondary)', margin: 0 }}>
                      Restricts file write and directory modification strictly within the designated workspace boundary. Path traversal attempts are blocked automatically by canonical path resolution.
                    </p>
                  </div>

                  {/* Approval Policy */}
                  <div
                    style={{
                      backgroundColor: 'var(--bg-primary)',
                      border: '1px solid var(--border-muted)',
                      borderRadius: 'var(--radius-sm)',
                      padding: '12px',
                    }}
                  >
                    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '6px' }}>
                      <span style={{ fontWeight: 600, fontSize: '12px', color: 'var(--text-primary)' }}>
                        Approval Policy
                      </span>
                      <span
                        style={{
                          fontFamily: 'var(--font-mono)',
                          fontSize: '11px',
                          padding: '2px 6px',
                          borderRadius: 'var(--radius-sm)',
                          backgroundColor: 'rgba(210, 153, 34, 0.15)',
                          color: 'var(--accent-orange)',
                          border: '1px solid rgba(210, 153, 34, 0.3)',
                        }}
                      >
                        {runtimeConfig?.approval_policy || 'on-request'}
                      </span>
                    </div>
                    <p style={{ fontSize: '11px', color: 'var(--text-secondary)', margin: 0 }}>
                      Commands categorized as potentially destructive or high-impact trigger an interactive approval dialog before execution is permitted.
                    </p>
                  </div>

                  {/* Dangerously Bypass */}
                  <div
                    style={{
                      backgroundColor: 'var(--bg-primary)',
                      border: '1px solid var(--border-muted)',
                      borderRadius: 'var(--radius-sm)',
                      padding: '12px',
                    }}
                  >
                    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '6px' }}>
                      <span style={{ fontWeight: 600, fontSize: '12px', color: 'var(--text-primary)' }}>
                        Safeguard Bypass Guard
                      </span>
                      <span
                        style={{
                          fontFamily: 'var(--font-mono)',
                          fontSize: '11px',
                          padding: '2px 6px',
                          borderRadius: 'var(--radius-sm)',
                          backgroundColor: runtimeConfig?.dangerously_bypass
                            ? 'rgba(218, 54, 51, 0.15)'
                            : 'rgba(46, 160, 67, 0.15)',
                          color: runtimeConfig?.dangerously_bypass
                            ? 'var(--accent-red)'
                            : 'var(--accent-green)',
                          border: `1px solid ${runtimeConfig?.dangerously_bypass ? 'rgba(218, 54, 51, 0.3)' : 'rgba(46, 160, 67, 0.3)'}`,
                        }}
                      >
                        {runtimeConfig?.dangerously_bypass ? 'Bypassed (Unsafe)' : 'Protected (Active)'}
                      </span>
                    </div>
                    <p style={{ fontSize: '11px', color: 'var(--text-secondary)', margin: 0 }}>
                      When enabled via command-line flags, standard approval requirements are bypassed. In normal operation, all safety barriers remain securely active.
                    </p>
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* TAB 4: Tool Catalog */}
          {activeTab === 'tools' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '10px' }}>
                <div>
                  <h4 style={{ fontSize: '13px', fontWeight: 600, color: 'var(--text-primary)', marginBottom: '2px' }}>
                    Registered Tool Catalog
                  </h4>
                  <p style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
                    Tools available to the assistant for inspection, file operations, and execution.
                  </p>
                </div>
                <div style={{ position: 'relative', width: '200px' }}>
                  <Search size={13} style={{ position: 'absolute', left: '8px', top: '7px', color: 'var(--text-muted)' }} />
                  <input
                    type="text"
                    value={toolSearch}
                    onChange={(e) => setToolSearch(e.target.value)}
                    placeholder="Filter tools..."
                    aria-label="Filter registered tools"
                    style={{
                      width: '100%',
                      padding: '4px 8px 4px 26px',
                      backgroundColor: 'var(--bg-primary)',
                      border: '1px solid var(--border-muted)',
                      borderRadius: 'var(--radius-sm)',
                      color: 'var(--text-primary)',
                      fontSize: '11px',
                    }}
                  />
                </div>
              </div>

              {filteredTools.length === 0 ? (
                <div
                  style={{
                    padding: '24px',
                    textAlign: 'center',
                    backgroundColor: 'var(--bg-primary)',
                    borderRadius: 'var(--radius-sm)',
                    border: '1px solid var(--border-muted)',
                    color: 'var(--text-secondary)',
                    fontSize: '12px',
                  }}
                >
                  {registeredTools.length === 0 ? 'No registered tools reported by runtime.' : 'No tools match filter.'}
                </div>
              ) : (
                <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', maxHeight: '360px', overflowY: 'auto' }}>
                  {filteredTools.map((tool) => {
                    const isSchemaOpen = !!expandedToolSchemas[tool.name];
                    return (
                      <div
                        key={tool.name}
                        style={{
                          backgroundColor: 'var(--bg-primary)',
                          border: '1px solid var(--border-muted)',
                          borderRadius: 'var(--radius-sm)',
                          padding: '10px 12px',
                          display: 'flex',
                          flexDirection: 'column',
                          gap: '6px',
                        }}
                      >
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                          <span
                            style={{
                              fontFamily: 'var(--font-mono)',
                              fontSize: '12px',
                              fontWeight: 600,
                              color: 'var(--accent-blue-hover)',
                            }}
                          >
                            {tool.name}
                          </span>
                          {tool.parameters_schema && (
                            <button
                              type="button"
                              onClick={() => toggleToolSchema(tool.name)}
                              className="btn btn-sm"
                              style={{
                                fontSize: '10px',
                                padding: '2px 8px',
                                display: 'flex',
                                alignItems: 'center',
                                gap: '4px',
                                background: 'transparent',
                              }}
                            >
                              {isSchemaOpen ? <ChevronDown size={11} /> : <ChevronRight size={11} />}
                              <span>{isSchemaOpen ? 'Hide Schema' : 'View Schema'}</span>
                            </button>
                          )}
                        </div>

                        <p style={{ fontSize: '11px', color: 'var(--text-secondary)', margin: 0, lineHeight: 1.4 }}>
                          {tool.description}
                        </p>

                        {isSchemaOpen && tool.parameters_schema && (
                          <pre
                            style={{
                              marginTop: '6px',
                              padding: '8px',
                              backgroundColor: '#070a0e',
                              borderRadius: 'var(--radius-sm)',
                              border: '1px solid var(--border-muted)',
                              fontFamily: 'var(--font-mono)',
                              fontSize: '10px',
                              color: 'var(--text-muted)',
                              maxHeight: '160px',
                              overflowY: 'auto',
                              whiteSpace: 'pre-wrap',
                            }}
                          >
                            {formatSchema(tool.parameters_schema)}
                          </pre>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          )}

          {/* TAB 5: Diagnostics & Audit */}
          {activeTab === 'audit' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                <div>
                  <h4 style={{ fontSize: '13px', fontWeight: 600, color: 'var(--text-primary)', marginBottom: '2px' }}>
                    Runtime Diagnostics & State
                  </h4>
                  <p style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
                    Authoritative health monitoring and active session metrics.
                  </p>
                </div>
                <button
                  type="button"
                  onClick={() => void handleRefreshDiagnostics()}
                  disabled={isRefreshing}
                  className="btn btn-sm"
                  style={{ fontSize: '11px', display: 'flex', alignItems: 'center', gap: '6px' }}
                >
                  <RefreshCw size={12} className={isRefreshing ? 'spin' : ''} style={{ animation: isRefreshing ? 'spin 1s linear infinite' : 'none' }} />
                  <span>Refresh</span>
                </button>
              </div>

              <div
                style={{
                  backgroundColor: 'var(--bg-primary)',
                  border: '1px solid var(--border-muted)',
                  borderRadius: 'var(--radius-sm)',
                  padding: '12px',
                  display: 'grid',
                  gridTemplateColumns: '150px 1fr',
                  rowGap: '8px',
                  fontSize: '12px',
                }}
              >
                <span style={{ color: 'var(--text-secondary)' }}>Operational State:</span>
                <span
                  style={{
                    color: runtimeStatus?.is_operational ? 'var(--accent-green)' : 'var(--accent-red)',
                    fontWeight: 600,
                  }}
                >
                  {runtimeStatus?.is_operational ? 'Operational (Ready for turns)' : 'Non-Operational'}
                </span>

                <span style={{ color: 'var(--text-secondary)' }}>Engine State:</span>
                <span style={{ fontFamily: 'var(--font-mono)' }}>{runtimeStatus?.state || 'Unknown'}</span>

                <span style={{ color: 'var(--text-secondary)' }}>Terminal Engine:</span>
                <span>{runtimeStatus?.is_terminal ? 'Yes' : 'No'}</span>

                <span style={{ color: 'var(--text-secondary)' }}>Active Session:</span>
                <span style={{ fontFamily: 'var(--font-mono)', wordBreak: 'break-all' }}>
                  {activeSessionId || 'None (Timeline clear)'}
                </span>

                <span style={{ color: 'var(--text-secondary)' }}>Session Messages:</span>
                <span>{messages.length}</span>

                <span style={{ color: 'var(--text-secondary)' }}>Pending Approvals:</span>
                <span style={{ color: pendingApprovals.length > 0 ? 'var(--accent-orange)' : 'var(--text-primary)' }}>
                  {pendingApprovals.length}
                </span>

                <span style={{ color: 'var(--text-secondary)' }}>Pending Questions:</span>
                <span style={{ color: pendingQuestions.length > 0 ? 'var(--accent-blue)' : 'var(--text-primary)' }}>
                  {pendingQuestions.length}
                </span>

                <span style={{ color: 'var(--text-secondary)' }}>Active Features:</span>
                <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap' }}>
                  {(runtimeConfig?.active_features || environmentInfo?.features || []).length > 0 ? (
                    (runtimeConfig?.active_features || environmentInfo?.features || []).map((feat) => (
                      <span
                        key={feat}
                        style={{
                          fontSize: '10px',
                          padding: '1px 6px',
                          borderRadius: 'var(--radius-sm)',
                          backgroundColor: 'var(--bg-tertiary)',
                          color: 'var(--text-secondary)',
                          fontFamily: 'var(--font-mono)',
                        }}
                      >
                        {feat}
                      </span>
                    ))
                  ) : (
                    <span style={{ color: 'var(--text-muted)' }}>Standard configuration</span>
                  )}
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div
          style={{
            display: 'flex',
            justifyContent: 'flex-end',
            padding: '12px 18px',
            borderTop: '1px solid var(--border-primary)',
            backgroundColor: 'var(--bg-secondary)',
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
