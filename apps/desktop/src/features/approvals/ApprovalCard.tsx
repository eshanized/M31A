import React, { useState } from 'react';
import { ShieldAlert, Check, X, Terminal } from 'lucide-react';
import type { PendingApprovalDto } from '../../types/ipc';
import { useApp } from '../../stores/AppContext';

export const ApprovalCard: React.FC<{ approval: PendingApprovalDto }> = ({
  approval,
}) => {
  const { resolveApproval } = useApp();
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [denyReason, setDenyReason] = useState('');
  const [showDenyInput, setShowDenyInput] = useState(false);

  const handleApprove = async () => {
    setIsSubmitting(true);
    try {
      await resolveApproval(approval.request_id, true);
    } catch (err) {
      console.error('Failed to approve request:', err);
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleDeny = async () => {
    setIsSubmitting(true);
    try {
      await resolveApproval(
        approval.request_id,
        false,
        denyReason.trim() || undefined
      );
    } catch (err) {
      console.error('Failed to deny request:', err);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div
      style={{
        margin: '12px 16px',
        padding: '12px 16px',
        borderRadius: 'var(--radius-md)',
        backgroundColor: 'rgba(210, 153, 34, 0.08)',
        border: '1px solid var(--accent-orange)',
        boxShadow: '0 4px 12px rgba(0, 0, 0, 0.2)',
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          marginBottom: '8px',
          color: 'var(--accent-orange)',
          fontWeight: 600,
          fontSize: '12px',
        }}
      >
        <ShieldAlert size={16} />
        <span>Authorization Required</span>
        <span
          style={{
            marginLeft: 'auto',
            fontSize: '10px',
            color: 'var(--text-secondary)',
            fontFamily: 'var(--font-mono)',
          }}
        >
          {approval.request_id.substring(0, 8)}
        </span>
      </div>

      <div style={{ fontSize: '12px', marginBottom: '8px', color: 'var(--text-primary)' }}>
        {approval.reason || 'The runtime execution engine requests permission to perform this command:'}
      </div>

      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '6px',
          padding: '6px 10px',
          borderRadius: 'var(--radius-sm)',
          backgroundColor: '#0a0d12',
          border: '1px solid var(--border-primary)',
          fontFamily: 'var(--font-mono)',
          fontSize: '11px',
          color: '#e6edf3',
          marginBottom: '10px',
          wordBreak: 'break-all',
        }}
      >
        <Terminal size={12} color="var(--text-secondary)" />
        <code>{approval.command}</code>
      </div>

      {showDenyInput && (
        <div style={{ marginBottom: '10px' }}>
          <input
            type="text"
            value={denyReason}
            onChange={(e) => setDenyReason(e.target.value)}
            placeholder="Reason for denial (optional)..."
            style={{
              width: '100%',
              padding: '6px 10px',
              borderRadius: 'var(--radius-sm)',
              border: '1px solid var(--border-primary)',
              backgroundColor: 'var(--bg-primary)',
              color: 'var(--text-primary)',
              fontSize: '12px',
            }}
          />
        </div>
      )}

      <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
        {showDenyInput ? (
          <>
            <button
              type="button"
              onClick={() => setShowDenyInput(false)}
              className="btn btn-sm"
              disabled={isSubmitting}
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={handleDeny}
              className="btn btn-danger btn-sm"
              disabled={isSubmitting}
            >
              <X size={12} /> Confirm Deny
            </button>
          </>
        ) : (
          <>
            <button
              type="button"
              onClick={() => setShowDenyInput(true)}
              className="btn btn-sm"
              disabled={isSubmitting}
            >
              <X size={12} /> Deny
            </button>
            <button
              type="button"
              onClick={handleApprove}
              className="btn btn-primary btn-sm"
              disabled={isSubmitting}
            >
              <Check size={12} /> Allow Once
            </button>
          </>
        )}
      </div>
    </div>
  );
};
