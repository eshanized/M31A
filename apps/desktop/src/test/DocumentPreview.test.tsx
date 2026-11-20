import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { DocxViewer } from '../features/documents/DocxViewer';
import { XlsxViewer } from '../features/documents/XlsxViewer';
import { PdfViewer } from '../features/documents/PdfViewer';
import type {
  DocxPreviewPayloadDto,
  XlsxPreviewPayloadDto,
  PdfPreviewPayloadDto,
} from '../types/ipc';

describe('Document Preview UI Components', () => {
  it('renders DocxViewer with headings, tables, and handles quote callback', () => {
    const onQuotePassage = vi.fn();
    const docxData: DocxPreviewPayloadDto = {
      file_name: 'test_spec.docx',
      total_paragraphs: 2,
      total_words: 8,
      blocks: [
        {
          kind: 'paragraph',
          data: {
            text: 'System Overview',
            style: 'Heading1',
            is_heading: true,
          },
        },
        {
          kind: 'paragraph',
          data: {
            text: 'M31A agent architecture documentation.',
            style: 'Normal',
            is_heading: false,
          },
        },
        {
          kind: 'table',
          data: {
            rows: [
              ['Crate', 'Feature'],
              ['m31a-cua', 'Accessibility CUA'],
            ],
          },
        },
      ],
    };

    render(<DocxViewer data={docxData} onQuotePassage={onQuotePassage} />);

    expect(screen.getByText('test_spec.docx')).toBeDefined();
    expect(screen.getByText('System Overview')).toBeDefined();
    expect(screen.getByText('M31A agent architecture documentation.')).toBeDefined();
    expect(screen.getByText('m31a-cua')).toBeDefined();
    expect(screen.getByText('Accessibility CUA')).toBeDefined();

    // Trigger quote button
    const quoteBtn = screen.getByTitle('Quote passage in chat');
    fireEvent.click(quoteBtn);
    expect(onQuotePassage).toHaveBeenCalledWith('M31A agent architecture documentation.');
  });

  it('renders XlsxViewer with multi-sheet tabs, column letters, and cell selection quote', () => {
    const onQuoteCell = vi.fn();
    const xlsxData: XlsxPreviewPayloadDto = {
      file_name: 'financials.xlsx',
      active_sheet_index: 0,
      sheets: [
        {
          name: 'Q1',
          max_columns: 2,
          rows: [
            ['Metric', 'Revenue'],
            ['Growth', '$1,200,000'],
          ],
        },
        {
          name: 'Q2',
          max_columns: 2,
          rows: [['Metric', 'Revenue']],
        },
      ],
    };

    render(<XlsxViewer data={xlsxData} onQuoteCell={onQuoteCell} />);

    expect(screen.getByText('financials.xlsx')).toBeDefined();
    expect(screen.getByText('Q1')).toBeDefined();
    expect(screen.getByText('Q2')).toBeDefined();
    expect(screen.getByText('Metric')).toBeDefined();
    expect(screen.getByText('$1,200,000')).toBeDefined();

    // Click cell to inspect and quote
    const cell = screen.getByText('$1,200,000');
    fireEvent.click(cell);

    const quoteBtn = screen.getByText('Quote in Chat');
    expect(quoteBtn).toBeDefined();
    fireEvent.click(quoteBtn);

    expect(onQuoteCell).toHaveBeenCalledWith('Q1!B2', '$1,200,000');
  });

  it('renders PdfViewer with title, author, page count, and stream preview', () => {
    const pdfData: PdfPreviewPayloadDto = {
      file_name: 'research_paper.pdf',
      page_count: 14,
      title: 'Autonomous Coding Agents at Scale',
      author: 'Eshan Roy',
      file_size_bytes: 1048576,
      text_preview: 'Abstract: We introduce M31A...',
    };

    render(<PdfViewer data={pdfData} />);

    expect(screen.getByText('research_paper.pdf')).toBeDefined();
    expect(screen.getByText('Autonomous Coding Agents at Scale')).toBeDefined();
    expect(screen.getByText('Eshan Roy')).toBeDefined();
    expect(screen.getByText('14 pages')).toBeDefined();
    expect(screen.getByText('1 MB')).toBeDefined();
    expect(screen.getByText('Abstract: We introduce M31A...')).toBeDefined();
  });
});
