import React, { useState } from 'react';
import { Check, Copy } from 'lucide-react';

interface CodeBlockProps {
  language?: string;
  code: string;
}

export const CodeBlock: React.FC<CodeBlockProps> = ({ language, code }) => {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy to clipboard:', err);
    }
  };

  const isDiff =
    language === 'diff' ||
    language === 'patch' ||
    code.startsWith('diff --git') ||
    (code.includes('--- ') && code.includes('+++ '));

  const renderCodeLines = () => {
    if (!isDiff) {
      return <code>{code}</code>;
    }

    const lines = code.split('\n');
    return (
      <div style={{ display: 'flex', flexDirection: 'column' }}>
        {lines.map((line, idx) => {
          let lineBg = 'transparent';
          let textColor = '#e6edf3';

          if (line.startsWith('+') && !line.startsWith('+++')) {
            lineBg = 'rgba(46, 160, 67, 0.18)';
            textColor = '#3fb950';
          } else if (line.startsWith('-') && !line.startsWith('---')) {
            lineBg = 'rgba(248, 81, 73, 0.18)';
            textColor = '#f85149';
          } else if (line.startsWith('@@')) {
            textColor = '#d2a8ff';
            lineBg = 'rgba(187, 128, 247, 0.08)';
          } else if (line.startsWith('diff') || line.startsWith('index') || line.startsWith('---') || line.startsWith('+++')) {
            textColor = '#58a6ff';
          }

          return (
            <div
              key={idx}
              style={{
                backgroundColor: lineBg,
                color: textColor,
                padding: '1px 8px',
                borderRadius: '2px',
                whiteSpace: 'pre',
              }}
            >
              {line || ' '}
            </div>
          );
        })}
      </div>
    );
  };

  return (
    <div
      style={{
        margin: '10px 0',
        borderRadius: 'var(--radius-md)',
        border: '1px solid var(--border-primary)',
        backgroundColor: '#05070a',
        overflow: 'hidden',
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '6px 12px',
          backgroundColor: 'var(--bg-secondary)',
          borderBottom: '1px solid var(--border-primary)',
          fontSize: '11px',
          color: 'var(--text-secondary)',
          fontFamily: 'var(--font-mono)',
        }}
      >
        <span style={{ fontWeight: 600 }}>{language || (isDiff ? 'diff' : 'text')}</span>
        <button
          type="button"
          onClick={handleCopy}
          className="btn btn-sm"
          style={{ padding: '2px 6px', fontSize: '10px' }}
          title="Copy code"
        >
          {copied ? (
            <>
              <Check size={12} color="var(--accent-green)" /> Copied
            </>
          ) : (
            <>
              <Copy size={12} /> Copy
            </>
          )}
        </button>
      </div>
      <pre
        style={{
          padding: '10px 12px',
          overflowX: 'auto',
          fontSize: '12px',
          lineHeight: '1.45',
          fontFamily: 'var(--font-mono)',
          color: '#e6edf3',
          margin: 0,
        }}
      >
        {renderCodeLines()}
      </pre>
    </div>
  );
};
