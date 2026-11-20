import React from 'react';
import type { DocxPreviewPayloadDto } from '../../types/ipc';
import { Quote, FileText } from 'lucide-react';

interface DocxViewerProps {
  data: DocxPreviewPayloadDto;
  onQuotePassage?: (text: string) => void;
}

export const DocxViewer: React.FC<DocxViewerProps> = ({ data, onQuotePassage }) => {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflowY: 'auto', padding: '16px', background: 'var(--bg-primary, #1e1e2e)', color: 'var(--text-primary, #cdd6f4)' }}>
      {/* Header bar */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', borderBottom: '1px solid var(--border-primary, #313244)', paddingBottom: '10px', marginBottom: '16px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <FileText size={18} color="var(--accent-blue, #89b4fa)" />
          <span style={{ fontWeight: 600, fontSize: '14px' }}>{data.file_name}</span>
        </div>
        <div style={{ display: 'flex', gap: '12px', fontSize: '11px', color: 'var(--text-secondary, #a6adc8)' }}>
          <span>{data.total_paragraphs} paragraphs</span>
          <span>•</span>
          <span>{data.total_words} words</span>
        </div>
      </div>

      {/* Document Body */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', maxWidth: '800px', margin: '0 auto', width: '100%', background: 'var(--bg-secondary, #181825)', padding: '24px', borderRadius: '8px', border: '1px solid var(--border-secondary, #313244)', boxShadow: '0 4px 12px rgba(0,0,0,0.2)' }}>
        {data.blocks.map((block, idx) => {
          if (block.kind === 'paragraph') {
            const p = block.data;
            if (p.is_heading) {
              return (
                <div key={idx} style={{ position: 'relative', group: 'p-item' } as any}>
                  <h2 style={{ fontSize: '18px', fontWeight: 700, color: 'var(--accent-blue, #89b4fa)', marginTop: '8px', marginBottom: '4px' }}>
                    {p.text}
                  </h2>
                </div>
              );
            }
            return (
              <div
                key={idx}
                style={{
                  display: 'flex',
                  alignItems: 'flex-start',
                  justifyContent: 'space-between',
                  gap: '8px',
                  lineHeight: '1.6',
                  fontSize: '13px',
                }}
              >
                <p style={{ margin: 0, flex: 1, whiteSpace: 'pre-wrap' }}>{p.text}</p>
                {onQuotePassage && (
                  <button
                    type="button"
                    onClick={() => onQuotePassage(p.text)}
                    title="Quote passage in chat"
                    style={{
                      background: 'transparent',
                      border: 'none',
                      cursor: 'pointer',
                      padding: '2px',
                      opacity: 0.6,
                      color: 'var(--text-secondary, #a6adc8)',
                    }}
                    onMouseEnter={(e) => (e.currentTarget.style.opacity = '1')}
                    onMouseLeave={(e) => (e.currentTarget.style.opacity = '0.6')}
                  >
                    <Quote size={13} />
                  </button>
                )}
              </div>
            );
          }

          if (block.kind === 'table') {
            const tbl = block.data;
            return (
              <div key={idx} style={{ margin: '12px 0', overflowX: 'auto' }}>
                <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '12px' }}>
                  <tbody>
                    {tbl.rows.map((row, rIdx) => (
                      <tr key={rIdx} style={{ background: rIdx === 0 ? 'var(--bg-tertiary, #313244)' : 'transparent' }}>
                        {row.map((cell, cIdx) => (
                          <td
                            key={cIdx}
                            style={{
                              border: '1px solid var(--border-primary, #45475a)',
                              padding: '6px 10px',
                              fontWeight: rIdx === 0 ? 600 : 400,
                              color: rIdx === 0 ? 'var(--accent-blue, #89b4fa)' : 'inherit',
                            }}
                          >
                            {cell}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            );
          }

          return null;
        })}
      </div>
    </div>
  );
};
