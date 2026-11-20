import React, { useState, useEffect } from 'react';
import {
  Folder,
  FolderOpen,
  FileText,
  FileCode,
  File,
  RefreshCw,
  Eye,
  X,
  Copy,
  Check,
  FolderPlus,
} from 'lucide-react';
import { useApp } from '../../stores/AppContext';
import type { WorkspaceFileEntryDto } from '../../types/ipc';
import { DocumentPreviewPanel } from '../documents/DocumentPreviewPanel';

export const WorkspaceExplorer: React.FC = () => {
  const {
    workspaceFiles,
    isLoadingWorkspaceFiles,
    fetchWorkspaceFiles,
    previewFilePath,
    previewFileContent,
    previewFile,
    pickWorkspaceFolder,
  } = useApp();

  const [currentSubpath, setCurrentSubpath] = useState<string>('');
  const [copiedPreview, setCopiedPreview] = useState<boolean>(false);
  const [searchFilter, setSearchFilter] = useState<string>('');

  useEffect(() => {
    void fetchWorkspaceFiles(currentSubpath);
  }, [currentSubpath, fetchWorkspaceFiles]);

  const handleFolderClick = (entry: WorkspaceFileEntryDto) => {
    if (entry.is_dir) {
      setCurrentSubpath(entry.path);
    }
  };

  const handleNavigateUp = () => {
    if (!currentSubpath) return;
    const parts = currentSubpath.split(/[\\/]/);
    parts.pop();
    setCurrentSubpath(parts.join('/'));
  };

  const formatFileSize = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
  };

  const getFileIcon = (entry: WorkspaceFileEntryDto) => {
    if (entry.is_dir) {
      return <Folder size={14} color="var(--accent-blue)" />;
    }
    const ext = entry.name.split('.').pop()?.toLowerCase();
    switch (ext) {
      case 'rs':
      case 'ts':
      case 'tsx':
      case 'js':
      case 'jsx':
      case 'py':
      case 'go':
      case 'c':
      case 'cpp':
        return <FileCode size={14} color="var(--accent-green)" />;
      case 'md':
      case 'json':
      case 'toml':
      case 'yaml':
      case 'yml':
      case 'txt':
        return <FileText size={14} color="var(--accent-orange)" />;
      default:
        return <File size={14} color="var(--text-secondary)" />;
    }
  };

  const handleCopyPreview = async () => {
    if (!previewFileContent) return;
    await navigator.clipboard.writeText(previewFileContent);
    setCopiedPreview(true);
    setTimeout(() => setCopiedPreview(false), 2000);
  };

  const isOfficeDocument = (path: string | null): boolean => {
    if (!path) return false;
    const ext = path.split('.').pop()?.toLowerCase();
    return ext === 'docx' || ext === 'xlsx' || ext === 'xls' || ext === 'pdf';
  };

  const filteredFiles = workspaceFiles.filter((f) =>
    f.name.toLowerCase().includes(searchFilter.toLowerCase())
  );

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        backgroundColor: 'var(--bg-secondary)',
        fontSize: '12px',
        userSelect: 'none',
      }}
    >
      {/* Explorer Header */}
      <div
        style={{
          padding: '10px 12px',
          borderBottom: '1px solid var(--border-primary)',
          display: 'flex',
          flexDirection: 'column',
          gap: '8px',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <span style={{ fontSize: '11px', fontWeight: 600, color: 'var(--text-secondary)', textTransform: 'uppercase' }}>
            Files Explorer
          </span>
          <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
            <button
              type="button"
              onClick={() => void fetchWorkspaceFiles(currentSubpath)}
              disabled={isLoadingWorkspaceFiles}
              className="btn btn-sm"
              style={{ padding: '3px 6px', background: 'transparent', border: 'none' }}
              title="Refresh directory"
            >
              <RefreshCw size={12} className={isLoadingWorkspaceFiles ? 'spin' : ''} />
            </button>
            <button
              type="button"
              onClick={() => void pickWorkspaceFolder()}
              className="btn btn-sm"
              style={{ padding: '3px 6px' }}
              title="Open folder..."
            >
              <FolderPlus size={12} /> Open
            </button>
          </div>
        </div>

        {/* Current Path Breadcrumb */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '6px',
            fontSize: '11px',
            color: 'var(--text-muted)',
            fontFamily: 'var(--font-mono)',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {currentSubpath ? (
            <button
              type="button"
              onClick={handleNavigateUp}
              className="btn btn-sm"
              style={{ padding: '2px 4px', fontSize: '10px' }}
              title="Navigate up one level"
            >
              ..
            </button>
          ) : (
            <FolderOpen size={12} color="var(--text-secondary)" />
          )}
          <span>/{currentSubpath || ''}</span>
        </div>

        {/* Filter input */}
        <input
          type="text"
          placeholder="Filter files..."
          aria-label="Filter workspace files"
          value={searchFilter}
          onChange={(e) => setSearchFilter(e.target.value)}
          style={{
            padding: '4px 8px',
            fontSize: '11px',
            borderRadius: 'var(--radius-sm)',
            border: '1px solid var(--border-primary)',
            backgroundColor: 'var(--bg-primary)',
            color: 'var(--text-primary)',
            outline: 'none',
          }}
        />
      </div>

      {/* Files List */}
      <div
        style={{
          flex: 1,
          overflowY: 'auto',
          padding: '4px',
          display: 'flex',
          flexDirection: 'column',
          gap: '1px',
        }}
      >
        {filteredFiles.length === 0 ? (
          <div style={{ padding: '24px 12px', textAlign: 'center', color: 'var(--text-muted)' }}>
            {isLoadingWorkspaceFiles ? 'Loading files...' : 'No files found in folder'}
          </div>
        ) : (
          filteredFiles.map((file) => (
            <div
              key={file.path}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '6px',
                padding: '6px 8px',
                borderRadius: 'var(--radius-sm)',
                cursor: 'pointer',
                backgroundColor: previewFilePath === file.path ? 'var(--bg-tertiary)' : 'transparent',
                color: 'var(--text-primary)',
                transition: 'background 0.15s ease',
              }}
              onClick={() => (file.is_dir ? handleFolderClick(file) : void previewFile(file.path))}
            >
              {getFileIcon(file)}
              <span
                style={{
                  flex: 1,
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                  fontWeight: file.is_dir ? 600 : 400,
                }}
                title={file.name}
              >
                {file.name}
              </span>

              {!file.is_dir && (
                <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                  <span style={{ fontSize: '10px', color: 'var(--text-muted)' }}>
                    {formatFileSize(file.size)}
                  </span>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      void previewFile(file.path);
                    }}
                    className="btn btn-sm"
                    style={{ padding: '2px 4px', background: 'transparent', border: 'none' }}
                    title="Preview file"
                  >
                    <Eye size={11} color="var(--text-muted)" />
                  </button>
                </div>
              )}
            </div>
          ))
        )}
      </div>

      {/* Embedded File Preview Panel if active */}
      {previewFilePath && (
        isOfficeDocument(previewFilePath) ? (
          <div
            style={{
              borderTop: '1px solid var(--border-primary)',
              height: '50%',
              display: 'flex',
              flexDirection: 'column',
              backgroundColor: '#0a0d12',
            }}
          >
            <DocumentPreviewPanel
              filePath={previewFilePath}
              onClose={() => void previewFile(null)}
            />
          </div>
        ) : (
          <div
            style={{
              borderTop: '1px solid var(--border-primary)',
              maxHeight: '40%',
              display: 'flex',
              flexDirection: 'column',
              backgroundColor: '#0a0d12',
            }}
          >
            <div
              style={{
                padding: '6px 10px',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                borderBottom: '1px solid var(--border-muted)',
                fontSize: '11px',
              }}
            >
              <span
                style={{
                  fontFamily: 'var(--font-mono)',
                  color: 'var(--accent-blue)',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                }}
                title={previewFilePath}
              >
                {previewFilePath.split(/[\\/]/).pop()}
              </span>
              <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                <button
                  type="button"
                  onClick={handleCopyPreview}
                  className="btn btn-sm"
                  style={{ padding: '2px 5px', fontSize: '10px' }}
                  title="Copy content"
                >
                  {copiedPreview ? <Check size={11} color="var(--accent-green)" /> : <Copy size={11} />}
                </button>
                <button
                  type="button"
                  onClick={() => void previewFile(null)}
                  className="btn btn-sm"
                  style={{ padding: '2px 4px', background: 'transparent', border: 'none' }}
                  title="Close preview"
                >
                  <X size={12} />
                </button>
              </div>
            </div>
            <pre
              style={{
                flex: 1,
                overflow: 'auto',
                padding: '8px',
                margin: 0,
                fontSize: '11px',
                fontFamily: 'var(--font-mono)',
                lineHeight: 1.4,
                color: 'var(--text-primary)',
                whiteSpace: 'pre',
              }}
            >
              {previewFileContent || 'Loading...'}
            </pre>
          </div>
        )
      )}
    </div>
  );
};
