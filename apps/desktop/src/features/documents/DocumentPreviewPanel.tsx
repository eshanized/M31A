import React, { useEffect, useState } from 'react';
import { ipc } from '../../lib/ipc';
import type { DocxPreviewPayloadDto, XlsxPreviewPayloadDto, PdfPreviewPayloadDto } from '../../types/ipc';
import { DocxViewer } from './DocxViewer';
import { XlsxViewer } from './XlsxViewer';
import { PdfViewer } from './PdfViewer';
import { X, Loader2, AlertCircle } from 'lucide-react';

interface DocumentPreviewPanelProps {
  filePath: string;
  onClose: () => void;
  onQuoteToChat?: (text: string) => void;
}

export const DocumentPreviewPanel: React.FC<DocumentPreviewPanelProps> = ({
  filePath,
  onClose,
  onQuoteToChat,
}) => {
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  const [docxData, setDocxData] = useState<DocxPreviewPayloadDto | null>(null);
  const [xlsxData, setXlsxData] = useState<XlsxPreviewPayloadDto | null>(null);
  const [pdfData, setPdfData] = useState<PdfPreviewPayloadDto | null>(null);

  const ext = filePath.split('.').pop()?.toLowerCase();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setDocxData(null);
    setXlsxData(null);
    setPdfData(null);

    async function loadDocument() {
      try {
        if (ext === 'docx') {
          const res = await ipc.previewDocxDocument(filePath);
          if (!cancelled) setDocxData(res);
        } else if (ext === 'xlsx' || ext === 'xls') {
          const res = await ipc.previewXlsxSpreadsheet(filePath);
          if (!cancelled) setXlsxData(res);
        } else if (ext === 'pdf') {
          const res = await ipc.previewPdfDocument(filePath);
          if (!cancelled) setPdfData(res);
        } else {
          throw new Error(`Unsupported document extension: .${ext}`);
        }
      } catch (err: any) {
        if (!cancelled) {
          setError(err?.message || String(err));
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    void loadDocument();
    return () => {
      cancelled = true;
    };
  }, [filePath, ext]);

  return (
    <div
      style={{
        position: 'relative',
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        width: '100%',
        background: 'var(--bg-primary, #1e1e2e)',
        overflow: 'hidden',
      }}
    >
      {/* Close button float */}
      <button
        type="button"
        onClick={onClose}
        style={{
          position: 'absolute',
          top: '12px',
          right: '12px',
          zIndex: 10,
          background: 'var(--bg-secondary, #313244)',
          border: '1px solid var(--border-primary, #45475a)',
          color: 'var(--text-primary, #cdd6f4)',
          borderRadius: '50%',
          width: '28px',
          height: '28px',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          cursor: 'pointer',
        }}
        title="Close Document Preview"
      >
        <X size={14} />
      </button>

      {loading && (
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%', gap: '8px', color: 'var(--text-secondary)' }}>
          <Loader2 size={20} className="animate-spin" />
          <span>Parsing {ext?.toUpperCase()} structure...</span>
        </div>
      )}

      {error && (
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%', gap: '12px', padding: '24px', textAlign: 'center' }}>
          <AlertCircle size={32} color="var(--accent-red, #f38ba8)" />
          <h4 style={{ margin: 0, color: 'var(--text-primary)' }}>Failed to load document preview</h4>
          <p style={{ margin: 0, fontSize: '12px', color: 'var(--text-secondary)', maxWidth: '500px' }}>{error}</p>
        </div>
      )}

      {!loading && !error && docxData && (
        <DocxViewer
          data={docxData}
          onQuotePassage={(text) => onQuoteToChat?.(`> "${text}"\n\n`)}
        />
      )}

      {!loading && !error && xlsxData && (
        <XlsxViewer
          data={xlsxData}
          onQuoteCell={(ref, val) => onQuoteToChat?.(`[${ref}: ${val}] `)}
        />
      )}

      {!loading && !error && pdfData && (
        <PdfViewer data={pdfData} />
      )}
    </div>
  );
};
