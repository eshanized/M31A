import React from 'react';
import type { PdfPreviewPayloadDto } from '../../types/ipc';
import { FileText, User, Hash, HardDrive } from 'lucide-react';

interface PdfViewerProps {
  data: PdfPreviewPayloadDto;
}

export const PdfViewer: React.FC<PdfViewerProps> = ({ data }) => {
  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflowY: 'auto', padding: '16px', background: 'var(--bg-primary, #1e1e2e)', color: 'var(--text-primary, #cdd6f4)' }}>
      {/* Header bar */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', borderBottom: '1px solid var(--border-primary, #313244)', paddingBottom: '10px', marginBottom: '16px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <FileText size={18} color="var(--accent-red, #f38ba8)" />
          <span style={{ fontWeight: 600, fontSize: '14px' }}>{data.file_name}</span>
        </div>
        <div style={{ display: 'flex', gap: '14px', fontSize: '12px', color: 'var(--text-secondary, #a6adc8)' }}>
          <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
            <Hash size={13} /> {data.page_count} {data.page_count === 1 ? 'page' : 'pages'}
          </span>
          <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
            <HardDrive size={13} /> {formatBytes(data.file_size_bytes)}
          </span>
          {data.author && (
            <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
              <User size={13} /> {data.author}
            </span>
          )}
        </div>
      </div>

      {/* PDF Metadata Card & Text Preview */}
      <div style={{ maxWidth: '800px', width: '100%', margin: '0 auto', display: 'flex', flexDirection: 'column', gap: '16px' }}>
        {data.title && (
          <div style={{ background: 'var(--bg-secondary, #181825)', padding: '14px 18px', borderRadius: '8px', border: '1px solid var(--border-primary, #313244)' }}>
            <span style={{ fontSize: '11px', textTransform: 'uppercase', color: 'var(--text-secondary, #a6adc8)', fontWeight: 600 }}>Document Title</span>
            <h3 style={{ margin: '4px 0 0 0', fontSize: '16px', color: 'var(--accent-blue, #89b4fa)' }}>{data.title}</h3>
          </div>
        )}

        <div style={{ background: 'var(--bg-secondary, #181825)', padding: '18px', borderRadius: '8px', border: '1px solid var(--border-primary, #313244)' }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '12px' }}>
            <span style={{ fontSize: '12px', fontWeight: 600, color: 'var(--text-secondary, #a6adc8)' }}>
              Extracted Text Stream Preview
            </span>
          </div>
          <pre
            style={{
              margin: 0,
              padding: '12px',
              background: 'var(--bg-tertiary, #11111b)',
              borderRadius: '6px',
              fontSize: '12px',
              fontFamily: 'var(--font-mono, monospace)',
              lineHeight: 1.5,
              whiteSpace: 'pre-wrap',
              maxHeight: '400px',
              overflowY: 'auto',
              color: 'var(--text-primary, #cdd6f4)',
            }}
          >
            {data.text_preview || 'No readable text stream detected in initial pages.'}
          </pre>
        </div>
      </div>
    </div>
  );
};
