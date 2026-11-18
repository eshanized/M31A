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
        <span>{language || 'text'}</span>
        <button
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
          padding: '12px',
          overflowX: 'auto',
          fontSize: '12px',
          lineHeight: '1.45',
          fontFamily: 'var(--font-mono)',
          color: '#e6edf3',
        }}
      >
        <code>{code}</code>
      </pre>
    </div>
  );
};
