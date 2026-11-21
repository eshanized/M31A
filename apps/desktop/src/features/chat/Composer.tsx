import React, { useState, useRef, useEffect, useCallback } from 'react';
import {
  Send,
  Square,
  Sparkles,
  Compass,
  ShieldCheck,
  Navigation,
  CheckCircle,
  GitCompare,
  ShieldAlert,
  Terminal,
  AtSign,
  Folder,
  FileCode,
  FileText,
} from 'lucide-react';
import { useApp } from '../../stores/AppContext';
import { FileMentionMatchDto } from '../../types/ipc';

interface SlashItem {
  name: string;
  display: string;
  description: string;
  category: string;
  usage?: string;
  isSubcommand?: boolean;
}

export const Composer: React.FC = () => {
  const {
    submitTurn,
    cancelActiveTurn,
    steerActiveTurn,
    runningTurnId,
    isSubmittingTurn,
    collaborationMode,
    setCollaborationMode,
    clearMessages,
    isRuntimeReady,
    slashCommands,
    searchMentionFiles,
  } = useApp();

  const [input, setInput] = useState('');
  const [steerInput, setSteerInput] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Slash commands popup state
  const [slashPopupOpen, setSlashPopupOpen] = useState(false);
  const [slashSelectedIndex, setSlashSelectedIndex] = useState(0);
  const activeSlashItemRef = useRef<HTMLDivElement>(null);

  // File mentions popup state
  const [mentionPopupOpen, setMentionPopupOpen] = useState(false);
  const [mentionQuery, setMentionQuery] = useState('');
  const [mentionStartIndex, setMentionStartIndex] = useState(0);
  const [mentionSelectedIndex, setMentionSelectedIndex] = useState(0);
  const [mentionMatches, setMentionMatches] = useState<FileMentionMatchDto[]>([]);
  const [isSearchingMentions, setIsSearchingMentions] = useState(false);
  const activeMentionItemRef = useRef<HTMLDivElement>(null);

  const isBusy = Boolean(runningTurnId) || isSubmittingTurn;

  // Auto focus input on mount and when turn finishes
  useEffect(() => {
    if (!isBusy && textareaRef.current) {
      textareaRef.current.focus();
    }
  }, [isBusy]);

  // Compute slash completion items
  const getSlashItems = useCallback((): SlashItem[] => {
    const trimmed = input.trimStart();
    const tokens = trimmed.split(/\s+/);
    if (tokens.length <= 1 && !trimmed.includes(' ')) {
      const filter = tokens[0]?.slice(1).toLowerCase() ?? '';
      return slashCommands
        .filter((cmd) => {
          if (!filter) return true;
          return (
            cmd.name.toLowerCase().includes(filter) ||
            cmd.description.toLowerCase().includes(filter) ||
            cmd.category.toLowerCase().includes(filter)
          );
        })
        .map((cmd) => ({
          name: cmd.name,
          display: cmd.name,
          description: cmd.description,
          category: cmd.category,
          usage: cmd.usage,
          isSubcommand: false,
        }));
    }

    // Subcommands for matched parent command
    const baseCmdName = tokens[0].toLowerCase();
    const parent = slashCommands.find(
      (c) =>
        c.name.toLowerCase() === baseCmdName ||
        c.name.toLowerCase() === `/${baseCmdName}`
    );
    if (parent && parent.subcommands && parent.subcommands.length > 0) {
      const subQuery =
        tokens.length > 1 && !input.endsWith(' ') ? tokens[1].toLowerCase() : '';
      return parent.subcommands
        .filter((sub) => !subQuery || sub.toLowerCase().includes(subQuery))
        .map((sub) => ({
          name: `${parent.name} ${sub}`,
          display: `${parent.name} ${sub}`,
          description: `Run ${parent.name} ${sub}`,
          category: parent.category,
          usage: `${parent.name} ${sub}`,
          isSubcommand: true,
        }));
    }

    return [];
  }, [input, slashCommands]);

  // Search workspace files when mention query changes
  useEffect(() => {
    if (!mentionPopupOpen) {
      setMentionMatches([]);
      return;
    }
    let active = true;
    setIsSearchingMentions(true);
    searchMentionFiles(mentionQuery)
      .then((results) => {
        if (active) {
          setMentionMatches(results.slice(0, 20));
          setMentionSelectedIndex(0);
        }
      })
      .catch((err) => {
        console.error('Mention file search failed:', err);
      })
      .finally(() => {
        if (active) setIsSearchingMentions(false);
      });

    return () => {
      active = false;
    };
  }, [mentionQuery, mentionPopupOpen, searchMentionFiles]);

  // Scroll active item into view
  useEffect(() => {
    if (activeSlashItemRef.current) {
      activeSlashItemRef.current.scrollIntoView({ block: 'nearest' });
    }
  }, [slashSelectedIndex]);

  useEffect(() => {
    if (activeMentionItemRef.current) {
      activeMentionItemRef.current.scrollIntoView({ block: 'nearest' });
    }
  }, [mentionSelectedIndex]);

  const handleSelectSlashCommand = (cmdName: string) => {
    setSlashPopupOpen(false);
    if (cmdName === '/clear') {
      clearMessages();
      setInput('');
      if (textareaRef.current) textareaRef.current.style.height = 'auto';
      return;
    }
    if (cmdName === '/plan') {
      setCollaborationMode('plan');
      setInput('');
      if (textareaRef.current) textareaRef.current.style.height = 'auto';
      return;
    }
    if (cmdName === '/review') {
      setCollaborationMode('review');
      setInput('');
      if (textareaRef.current) textareaRef.current.style.height = 'auto';
      return;
    }
    if (cmdName === '/default') {
      setCollaborationMode('default');
      setInput('');
      if (textareaRef.current) textareaRef.current.style.height = 'auto';
      return;
    }

    const next = `${cmdName} `;
    setInput(next);
    requestAnimationFrame(() => {
      if (textareaRef.current) {
        textareaRef.current.focus();
        textareaRef.current.setSelectionRange(next.length, next.length);
      }
    });
  };

  const handleSelectMention = (match: FileMentionMatchDto) => {
    setMentionPopupOpen(false);
    const cursorPos = textareaRef.current?.selectionStart ?? input.length;
    const textBefore = input.slice(0, mentionStartIndex);
    const textAfter = input.slice(cursorPos);
    const inserted = `@${match.path} `;
    const nextInput = `${textBefore}${inserted}${textAfter}`;
    setInput(nextInput);
    const nextCursor = textBefore.length + inserted.length;
    requestAnimationFrame(() => {
      if (textareaRef.current) {
        textareaRef.current.focus();
        textareaRef.current.setSelectionRange(nextCursor, nextCursor);
      }
    });
  };

  const handleSubmit = async () => {
    const trimmed = input.trim();
    if (!trimmed || isBusy || !isRuntimeReady) return;

    if (trimmed === '/clear') {
      clearMessages();
      setInput('');
      if (textareaRef.current) textareaRef.current.style.height = 'auto';
      return;
    }
    if (trimmed === '/plan') {
      setCollaborationMode('plan');
      setInput('');
      if (textareaRef.current) textareaRef.current.style.height = 'auto';
      return;
    }
    if (trimmed === '/review') {
      setCollaborationMode('review');
      setInput('');
      if (textareaRef.current) textareaRef.current.style.height = 'auto';
      return;
    }
    if (trimmed === '/default') {
      setCollaborationMode('default');
      setInput('');
      if (textareaRef.current) textareaRef.current.style.height = 'auto';
      return;
    }

    setSlashPopupOpen(false);
    setMentionPopupOpen(false);
    const submitted = await submitTurn(trimmed);
    if (submitted) {
      setInput('');
      if (textareaRef.current) {
        textareaRef.current.style.height = 'auto';
      }
    }
  };

  const handleSteer = async () => {
    const trimmed = steerInput.trim();
    if (!trimmed || !runningTurnId) return;
    await steerActiveTurn(trimmed);
    setSteerInput('');
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.nativeEvent.isComposing || e.keyCode === 229) return;

    // File mentions popup navigation
    if (mentionPopupOpen && mentionMatches.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setMentionSelectedIndex((prev) => (prev + 1) % mentionMatches.length);
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        setMentionSelectedIndex(
          (prev) => (prev - 1 + mentionMatches.length) % mentionMatches.length
        );
        return;
      }
      if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault();
        handleSelectMention(mentionMatches[mentionSelectedIndex]);
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        setMentionPopupOpen(false);
        return;
      }
    }

    // Slash command popup navigation
    const slashItems = getSlashItems();
    if (slashPopupOpen && slashItems.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSlashSelectedIndex((prev) => (prev + 1) % slashItems.length);
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSlashSelectedIndex(
          (prev) => (prev - 1 + slashItems.length) % slashItems.length
        );
        return;
      }
      if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault();
        handleSelectSlashCommand(slashItems[slashSelectedIndex].name);
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        setSlashPopupOpen(false);
        return;
      }
    }

    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value;
    const cursorPos = e.target.selectionStart ?? val.length;
    setInput(val);
    e.target.style.height = 'auto';
    e.target.style.height = `${Math.min(e.target.scrollHeight, 180)}px`;

    // Check slash commands trigger
    const trimmedStart = val.trimStart();
    if (trimmedStart.startsWith('/') && !trimmedStart.includes('\n')) {
      setSlashPopupOpen(true);
      setMentionPopupOpen(false);
      setSlashSelectedIndex(0);
    } else {
      setSlashPopupOpen(false);
    }

    // Check file mentions trigger
    const textBefore = val.slice(0, cursorPos);
    const atPos = textBefore.lastIndexOf('@');
    if (atPos !== -1) {
      const isStart = atPos === 0 || /\s/.test(textBefore[atPos - 1]);
      const query = textBefore.slice(atPos + 1);
      if (isStart && !/\s/.test(query)) {
        setMentionQuery(query);
        setMentionStartIndex(atPos);
        setMentionSelectedIndex(0);
        setMentionPopupOpen(true);
        setSlashPopupOpen(false);
        return;
      }
    }
    setMentionPopupOpen(false);
  };

  const triggerSlash = () => {
    setInput('/');
    setSlashPopupOpen(true);
    setMentionPopupOpen(false);
    setSlashSelectedIndex(0);
    requestAnimationFrame(() => {
      if (textareaRef.current) {
        textareaRef.current.focus();
        textareaRef.current.setSelectionRange(1, 1);
      }
    });
  };

  const triggerMention = () => {
    const current = input;
    const prefix = current.length > 0 && !current.endsWith(' ') ? ' @' : '@';
    const next = `${current}${prefix}`;
    setInput(next);
    setMentionQuery('');
    setMentionStartIndex(next.length - 1);
    setMentionPopupOpen(true);
    setSlashPopupOpen(false);
    requestAnimationFrame(() => {
      if (textareaRef.current) {
        textareaRef.current.focus();
        textareaRef.current.setSelectionRange(next.length, next.length);
      }
    });
  };

  const applyQuickPrompt = (prompt: string) => {
    setInput(prompt);
    if (textareaRef.current) {
      textareaRef.current.focus();
    }
  };

  const getFileIcon = (match: FileMentionMatchDto) => {
    if (match.is_dir) {
      return <Folder size={13} color="var(--accent-blue)" />;
    }
    const ext = match.filename.split('.').pop()?.toLowerCase();
    if (['rs', 'ts', 'tsx', 'js', 'jsx', 'py', 'go', 'c', 'cpp', 'h'].includes(ext ?? '')) {
      return <FileCode size={13} color="var(--accent-green)" />;
    }
    return <FileText size={13} color="var(--text-secondary)" />;
  };

  const slashItems = getSlashItems();

  return (
    <div
      style={{
        position: 'relative',
        padding: '12px 16px',
        borderTop: '1px solid var(--border-primary)',
        backgroundColor: 'var(--bg-secondary)',
      }}
    >
      {/* Autocomplete Popup for Slash Commands */}
      {slashPopupOpen && slashItems.length > 0 && (
        <div
          data-testid="slash-commands-popup"
          style={{
            position: 'absolute',
            bottom: '100%',
            left: '16px',
            right: '16px',
            marginBottom: '6px',
            maxHeight: '260px',
            overflowY: 'auto',
            backgroundColor: 'var(--bg-secondary)',
            border: '1px solid var(--border-primary)',
            borderRadius: 'var(--radius-md)',
            boxShadow: '0 8px 24px rgba(0, 0, 0, 0.45)',
            zIndex: 100,
            padding: '4px',
          }}
        >
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              padding: '6px 8px',
              fontSize: '11px',
              borderBottom: '1px solid var(--border-muted)',
              color: 'var(--text-secondary)',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '5px' }}>
              <Terminal size={12} color="var(--accent-blue)" />
              <span style={{ fontWeight: 600 }}>Commands</span>
            </div>
            <span style={{ fontSize: '10px', color: 'var(--text-muted)' }}>
              ↑↓ navigate • Enter/Tab select • Esc dismiss
            </span>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '2px', paddingTop: '4px' }}>
            {slashItems.map((item, idx) => {
              const isSelected = idx === slashSelectedIndex;
              return (
                <div
                  key={item.display}
                  ref={isSelected ? activeSlashItemRef : null}
                  onMouseEnter={() => setSlashSelectedIndex(idx)}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    handleSelectSlashCommand(item.name);
                  }}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    padding: '6px 8px',
                    borderRadius: 'var(--radius-sm)',
                    cursor: 'pointer',
                    backgroundColor: isSelected ? 'var(--bg-tertiary)' : 'transparent',
                    borderLeft: isSelected
                      ? '3px solid var(--accent-blue)'
                      : '3px solid transparent',
                    gap: '8px',
                  }}
                >
                  <div
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: '8px',
                      overflow: 'hidden',
                      minWidth: 0,
                    }}
                  >
                    <Terminal
                      size={12}
                      color={isSelected ? 'var(--accent-blue)' : 'var(--text-secondary)'}
                    />
                    <span
                      style={{
                        fontFamily: 'var(--font-mono)',
                        fontWeight: 600,
                        color: isSelected ? 'var(--accent-blue)' : 'var(--text-primary)',
                        fontSize: '12px',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {item.display}
                    </span>
                    <span
                      style={{
                        fontSize: '11px',
                        color: 'var(--text-secondary)',
                        whiteSpace: 'nowrap',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                      }}
                    >
                      {item.description}
                    </span>
                  </div>
                  <span
                    style={{
                      fontSize: '10px',
                      color: 'var(--text-muted)',
                      padding: '1px 5px',
                      borderRadius: '3px',
                      border: '1px solid var(--border-muted)',
                      whiteSpace: 'nowrap',
                    }}
                  >
                    {item.category}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Autocomplete Popup for File Mentions */}
      {mentionPopupOpen && (
        <div
          data-testid="file-mentions-popup"
          style={{
            position: 'absolute',
            bottom: '100%',
            left: '16px',
            right: '16px',
            marginBottom: '6px',
            maxHeight: '260px',
            overflowY: 'auto',
            backgroundColor: 'var(--bg-secondary)',
            border: '1px solid var(--border-primary)',
            borderRadius: 'var(--radius-md)',
            boxShadow: '0 8px 24px rgba(0, 0, 0, 0.45)',
            zIndex: 100,
            padding: '4px',
          }}
        >
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              padding: '6px 8px',
              fontSize: '11px',
              borderBottom: '1px solid var(--border-muted)',
              color: 'var(--text-secondary)',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '5px' }}>
              <AtSign size={12} color="var(--accent-green)" />
              <span style={{ fontWeight: 600 }}>Mention Workspace File</span>
              {isSearchingMentions && (
                <span
                  style={{
                    fontSize: '10px',
                    color: 'var(--accent-blue)',
                    marginLeft: '4px',
                  }}
                >
                  Searching...
                </span>
              )}
            </div>
            <span style={{ fontSize: '10px', color: 'var(--text-muted)' }}>
              ↑↓ navigate • Enter/Tab insert • Esc dismiss
            </span>
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: '2px', paddingTop: '4px' }}>
            {mentionMatches.length === 0 && !isSearchingMentions ? (
              <div
                style={{
                  padding: '12px 8px',
                  textAlign: 'center',
                  color: 'var(--text-muted)',
                  fontSize: '11px',
                }}
              >
                No files found matching &quot;@{mentionQuery}&quot;
              </div>
            ) : (
              mentionMatches.map((match, idx) => {
                const isSelected = idx === mentionSelectedIndex;
                return (
                  <div
                    key={match.path}
                    ref={isSelected ? activeMentionItemRef : null}
                    onMouseEnter={() => setMentionSelectedIndex(idx)}
                    onMouseDown={(e) => {
                      e.preventDefault();
                      handleSelectMention(match);
                    }}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'space-between',
                      padding: '6px 8px',
                      borderRadius: 'var(--radius-sm)',
                      cursor: 'pointer',
                      backgroundColor: isSelected ? 'var(--bg-tertiary)' : 'transparent',
                      borderLeft: isSelected
                        ? '3px solid var(--accent-green)'
                        : '3px solid transparent',
                      gap: '8px',
                    }}
                  >
                    <div
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: '8px',
                        overflow: 'hidden',
                        minWidth: 0,
                      }}
                    >
                      {getFileIcon(match)}
                      <span
                        style={{
                          fontWeight: 600,
                          color: isSelected ? 'var(--text-primary)' : 'var(--text-primary)',
                          fontSize: '12px',
                          whiteSpace: 'nowrap',
                        }}
                      >
                        {match.filename}
                      </span>
                      {match.path !== match.filename && (
                        <span
                          style={{
                            fontFamily: 'var(--font-mono)',
                            fontSize: '11px',
                            color: 'var(--text-muted)',
                            whiteSpace: 'nowrap',
                            overflow: 'hidden',
                            textOverflow: 'ellipsis',
                          }}
                        >
                          {match.path}
                        </span>
                      )}
                    </div>
                    {match.is_dir && (
                      <span
                        style={{
                          fontSize: '10px',
                          color: 'var(--accent-blue)',
                          padding: '1px 5px',
                          borderRadius: '3px',
                          border: '1px solid var(--border-muted)',
                          whiteSpace: 'nowrap',
                        }}
                      >
                        dir
                      </span>
                    )}
                  </div>
                );
              })
            )}
          </div>
        </div>
      )}

      {/* Collaboration Mode Bar & Quick Actions */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          marginBottom: '8px',
          fontSize: '11px',
          gap: '8px',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
          <span style={{ color: 'var(--text-secondary)', fontWeight: 500 }}>Mode:</span>
          <button
            type="button"
            onClick={() => setCollaborationMode('default')}
            className={`btn btn-sm ${collaborationMode === 'default' ? 'btn-primary' : ''}`}
            style={{ padding: '2px 8px' }}
            title="Standard autonomous execution"
          >
            <Sparkles size={11} /> Default
          </button>
          <button
            type="button"
            onClick={() => setCollaborationMode('plan')}
            className={`btn btn-sm ${collaborationMode === 'plan' ? 'btn-primary' : ''}`}
            style={{ padding: '2px 8px' }}
            title="Read-only planning and design mode"
          >
            <Compass size={11} /> Plan
          </button>
          <button
            type="button"
            onClick={() => setCollaborationMode('review')}
            className={`btn btn-sm ${collaborationMode === 'review' ? 'btn-primary' : ''}`}
            style={{ padding: '2px 8px' }}
            title="Audit and verification review mode"
          >
            <ShieldCheck size={11} /> Review
          </button>

          <span
            style={{
              width: '1px',
              height: '14px',
              backgroundColor: 'var(--border-primary)',
              margin: '0 4px',
            }}
          />

          {/* Quick Triggers for Slash and Mentions */}
          <button
            type="button"
            onClick={triggerSlash}
            className="btn btn-sm"
            style={{ padding: '2px 8px', fontSize: '11px' }}
            title="Open Slash Commands autocomplete"
          >
            <Terminal size={11} color="var(--accent-blue)" /> / Command
          </button>
          <button
            type="button"
            onClick={triggerMention}
            className="btn btn-sm"
            style={{ padding: '2px 8px', fontSize: '11px' }}
            title="Mention workspace file (@)"
          >
            <AtSign size={11} color="var(--accent-green)" /> @ File
          </button>
        </div>

        {/* Quick Prompt Pills when not busy and input is empty */}
        {!isBusy && !input && (
          <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
            <button
              type="button"
              onClick={() => applyQuickPrompt('Review workspace git status and diff.')}
              className="btn btn-sm"
              style={{ padding: '2px 6px', fontSize: '10px' }}
            >
              <GitCompare size={10} /> Git Diff
            </button>
            <button
              type="button"
              onClick={() => applyQuickPrompt('Verify workspace integrity and run all tests.')}
              className="btn btn-sm"
              style={{ padding: '2px 6px', fontSize: '10px' }}
            >
              <CheckCircle size={10} /> Verify All
            </button>
            <button
              type="button"
              onClick={() => applyQuickPrompt('Perform security audit and sandbox boundary review.')}
              className="btn btn-sm"
              style={{ padding: '2px 6px', fontSize: '10px' }}
            >
              <ShieldAlert size={10} /> Audit
            </button>
          </div>
        )}
      </div>

      {/* When turn is running: Live Steering Bar */}
      {isBusy && runningTurnId ? (
        <div
          style={{
            display: 'flex',
            gap: '8px',
            alignItems: 'center',
            borderRadius: 'var(--radius-md)',
            border: '1px solid var(--accent-blue)',
            backgroundColor: 'var(--bg-primary)',
            padding: '8px 12px',
          }}
        >
          <Navigation size={14} color="var(--accent-blue)" />
          <input
            type="text"
            placeholder="Steer active turn in real time (press Enter to steer)..."
            value={steerInput}
            onChange={(e) => setSteerInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                void handleSteer();
              }
            }}
            style={{
              flex: 1,
              border: 'none',
              outline: 'none',
              backgroundColor: 'transparent',
              fontSize: '12px',
              color: 'var(--text-primary)',
            }}
          />
          <button
            type="button"
            onClick={handleSteer}
            disabled={!steerInput.trim()}
            className="btn btn-primary btn-sm"
            style={{ padding: '4px 10px', height: '28px' }}
            title="Send steering instruction"
          >
            Steer
          </button>
          <button
            type="button"
            onClick={cancelActiveTurn}
            className="btn btn-danger btn-sm"
            style={{ padding: '4px 10px', height: '28px' }}
            title="Cancel running turn"
          >
            <Square size={11} fill="currentColor" /> Stop
          </button>
        </div>
      ) : (
        /* Standard Turn Submission Input */
        <div
          style={{
            display: 'flex',
            gap: '8px',
            alignItems: 'flex-end',
            borderRadius: 'var(--radius-md)',
            border: '1px solid var(--border-primary)',
            backgroundColor: 'var(--bg-primary)',
            padding: '8px 12px',
          }}
        >
          <textarea
            ref={textareaRef}
            value={input}
            onChange={handleInput}
            onKeyDown={handleKeyDown}
            placeholder={
              !isRuntimeReady
                ? 'Runtime initializing or unavailable...'
                : isBusy
                ? 'Turn in progress...'
                : 'Ask M31A to implement, debug, or refactor (/ for commands, @ for files, Enter to send)...'
            }
            disabled={!isRuntimeReady}
            rows={1}
            style={{
              flex: 1,
              border: 'none',
              outline: 'none',
              backgroundColor: 'transparent',
              resize: 'none',
              fontSize: '13px',
              color: 'var(--text-primary)',
              maxHeight: '180px',
              lineHeight: 1.5,
            }}
          />

          <button
            type="button"
            onClick={handleSubmit}
            disabled={!input.trim() || !isRuntimeReady || isBusy}
            className="btn btn-primary btn-sm"
            style={{ padding: '6px 12px', height: '32px' }}
            title="Submit turn"
          >
            <Send size={13} /> Send
          </button>
        </div>
      )}
    </div>
  );
};
