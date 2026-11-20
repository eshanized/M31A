import React, { useState } from 'react';
import type { XlsxPreviewPayloadDto } from '../../types/ipc';
import { Table, Quote, Layers } from 'lucide-react';

interface XlsxViewerProps {
  data: XlsxPreviewPayloadDto;
  onQuoteCell?: (reference: string, value: string) => void;
}

export const XlsxViewer: React.FC<XlsxViewerProps> = ({ data, onQuoteCell }) => {
  const [activeSheetIdx, setActiveSheetIdx] = useState<number>(data.active_sheet_index || 0);
  const [selectedCell, setSelectedCell] = useState<{ r: number; c: number; ref: string; val: string } | null>(null);

  const currentSheet = data.sheets[activeSheetIdx] || data.sheets[0];

  const getColHeader = (idx: number): string => {
    let name = '';
    let curr = idx;
    while (curr >= 0) {
      name = String.fromCharCode(65 + (curr % 26)) + name;
      curr = Math.floor(curr / 26) - 1;
    }
    return name;
  };

  const handleCellClick = (rIdx: number, cIdx: number, val: string) => {
    const colName = getColHeader(cIdx);
    const cellRef = `${currentSheet?.name || 'Sheet'}!${colName}${rIdx + 1}`;
    setSelectedCell({ r: rIdx, c: cIdx, ref: cellRef, val });
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden', background: 'var(--bg-primary, #1e1e2e)', color: 'var(--text-primary, #cdd6f4)' }}>
      {/* Top Header & Formula / Cell Bar */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '10px 16px', borderBottom: '1px solid var(--border-primary, #313244)', background: 'var(--bg-secondary, #181825)' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <Table size={18} color="var(--accent-green, #a6e3a1)" />
          <span style={{ fontWeight: 600, fontSize: '13px' }}>{data.file_name}</span>
        </div>
        {selectedCell ? (
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
            <span style={{ fontWeight: 600, color: 'var(--accent-blue, #89b4fa)', fontSize: '12px' }}>
              {selectedCell.ref}
            </span>
            <span style={{ fontSize: '12px', background: 'var(--bg-tertiary, #313244)', padding: '2px 8px', borderRadius: '4px' }}>
              {selectedCell.val}
            </span>
            {onQuoteCell && (
              <button
                type="button"
                onClick={() => onQuoteCell(selectedCell.ref, selectedCell.val)}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '4px',
                  background: 'var(--accent-blue, #89b4fa)',
                  color: '#11111b',
                  border: 'none',
                  borderRadius: '4px',
                  padding: '3px 8px',
                  fontSize: '11px',
                  fontWeight: 600,
                  cursor: 'pointer',
                }}
              >
                <Quote size={11} /> Quote in Chat
              </button>
            )}
          </div>
        ) : (
          <span style={{ fontSize: '11px', color: 'var(--text-secondary, #a6adc8)' }}>
            Click any cell to inspect or quote
          </span>
        )}
      </div>

      {/* Spreadsheet Grid */}
      <div style={{ flex: 1, overflow: 'auto', position: 'relative' }}>
        {currentSheet ? (
          <table style={{ borderCollapse: 'collapse', width: '100%', fontSize: '12px', tableLayout: 'fixed' }}>
            <thead>
              <tr style={{ background: 'var(--bg-tertiary, #313244)', position: 'sticky', top: 0, zIndex: 2 }}>
                <th style={{ width: '45px', border: '1px solid var(--border-primary, #45475a)', padding: '4px', textAlign: 'center', color: 'var(--text-secondary, #a6adc8)', fontWeight: 600 }}>#</th>
                {Array.from({ length: currentSheet.max_columns || 1 }).map((_, cIdx) => (
                  <th key={cIdx} style={{ minWidth: '100px', border: '1px solid var(--border-primary, #45475a)', padding: '4px 8px', textAlign: 'center', color: 'var(--text-secondary, #a6adc8)', fontWeight: 600 }}>
                    {getColHeader(cIdx)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {currentSheet.rows.map((row, rIdx) => (
                <tr key={rIdx}>
                  <td style={{ border: '1px solid var(--border-primary, #45475a)', padding: '4px', textAlign: 'center', background: 'var(--bg-secondary, #181825)', color: 'var(--text-secondary, #a6adc8)', fontWeight: 600, position: 'sticky', left: 0, zIndex: 1 }}>
                    {rIdx + 1}
                  </td>
                  {Array.from({ length: currentSheet.max_columns || 1 }).map((_, cIdx) => {
                    const cellVal = row[cIdx] || '';
                    const isSelected = selectedCell?.r === rIdx && selectedCell?.c === cIdx;
                    return (
                      <td
                        key={cIdx}
                        onClick={() => handleCellClick(rIdx, cIdx, cellVal)}
                        style={{
                          border: isSelected ? '2px solid var(--accent-blue, #89b4fa)' : '1px solid var(--border-secondary, #313244)',
                          padding: '4px 8px',
                          cursor: 'pointer',
                          background: isSelected ? 'rgba(137, 180, 250, 0.15)' : 'transparent',
                          whiteSpace: 'nowrap',
                          overflow: 'hidden',
                          textOverflow: 'ellipsis',
                        }}
                      >
                        {cellVal}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <div style={{ padding: '24px', textAlign: 'center', color: 'var(--text-secondary)' }}>
            No sheets found in workbook.
          </div>
        )}
      </div>

      {/* Bottom Sheet Selector Tabs */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '4px', padding: '6px 12px', borderTop: '1px solid var(--border-primary, #313244)', background: 'var(--bg-secondary, #181825)' }}>
        <Layers size={14} color="var(--text-secondary, #a6adc8)" />
        {data.sheets.map((sheet, idx) => (
          <button
            key={idx}
            type="button"
            onClick={() => {
              setActiveSheetIdx(idx);
              setSelectedCell(null);
            }}
            style={{
              padding: '4px 12px',
              border: 'none',
              borderRadius: '4px',
              fontSize: '12px',
              fontWeight: activeSheetIdx === idx ? 600 : 400,
              cursor: 'pointer',
              background: activeSheetIdx === idx ? 'var(--bg-tertiary, #313244)' : 'transparent',
              color: activeSheetIdx === idx ? 'var(--accent-blue, #89b4fa)' : 'var(--text-secondary, #a6adc8)',
            }}
          >
            {sheet.name}
          </button>
        ))}
      </div>
    </div>
  );
};
