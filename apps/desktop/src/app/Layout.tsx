import React, { useState } from 'react';
import { WorkspaceHeader } from '../features/workspace/WorkspaceHeader';
import { SessionSidebar } from '../features/sessions/SessionSidebar';
import { ChatTimeline } from '../features/chat/ChatTimeline';
import { Composer } from '../features/chat/Composer';
import { ApprovalCard } from '../features/approvals/ApprovalCard';
import { QuestionDialog } from '../features/approvals/QuestionDialog';
import { SettingsModal } from '../features/settings/SettingsModal';
import { useApp } from '../stores/AppContext';

export const Layout: React.FC = () => {
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const { pendingApprovals, pendingQuestions } = useApp();

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        height: '100vh',
        width: '100vw',
        backgroundColor: 'var(--bg-primary)',
        color: 'var(--text-primary)',
      }}
    >
      <WorkspaceHeader
        isSidebarOpen={isSidebarOpen}
        onToggleSidebar={() => setIsSidebarOpen(!isSidebarOpen)}
        onOpenSettings={() => setIsSettingsOpen(true)}
      />

      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        <SessionSidebar isOpen={isSidebarOpen} />

        <main
          style={{
            flex: 1,
            display: 'flex',
            flexDirection: 'column',
            backgroundColor: 'var(--bg-primary)',
            minWidth: 0,
            position: 'relative',
          }}
        >
          {/* Floating or pinned authorization and clarification cards */}
          {pendingApprovals.length > 0 && (
            <div style={{ display: 'flex', flexDirection: 'column' }}>
              {pendingApprovals.map((appr) => (
                <ApprovalCard key={appr.request_id} approval={appr} />
              ))}
            </div>
          )}

          {pendingQuestions.length > 0 && (
            <div style={{ display: 'flex', flexDirection: 'column' }}>
              {pendingQuestions.map((q) => (
                <QuestionDialog key={q.question_id} question={q} />
              ))}
            </div>
          )}

          {/* Central conversation timeline */}
          <ChatTimeline />

          {/* Interactive message composer */}
          <Composer />
        </main>
      </div>

      <SettingsModal
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
      />
    </div>
  );
};
