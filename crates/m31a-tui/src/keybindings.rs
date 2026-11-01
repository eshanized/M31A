//! Canonical Keybinding Registry & Specification (`m31a-tui::keybindings`).
//!
//! Provides the centralized single source of truth for all keyboard shortcuts, modal precedence,
//! and help documentation in the M31A interactive terminal.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    // Level 0: Global Emergency & Lifecycle
    EmergencyInterrupt,
    Quit,
    Help,
    CommandPalette,

    // Level 1: Core Navigation & Session
    ModelPicker,
    Settings,
    CodeModeScratchpad,
    TimingDashboard,
    ConcurrencyLocks,
    CycleSandboxMode,
    CycleApprovalMode,
    TogglePlanMode,
    UndoCheckpoint,
    ClearTranscript,
    GitDiff,
    CopyCodeBlock,
    Worktrees,
    ResumeSession,
    NewSession,
    ToggleInspector,
    PanelNextTab,
    PanelPrevTab,
    Analytics,

    // Level 2: Subagent Tabs
    SubagentNextTab,
    SubagentPrevTab,
    SubagentSelectTab(usize),

    // Level 3: Composer & Editing
    SubmitPrompt,
    QueuePrompt,
    InsertNewline,
    DismissModal,
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keybinding {
    pub key: &'static str,
    pub description: &'static str,
    pub category: &'static str,
    pub action: KeyAction,
}

pub const CANONICAL_KEYBINDINGS: &[Keybinding] = &[
    // Level 0: Global Emergency & Lifecycle
    Keybinding {
        key: "Ctrl+C",
        description: "Interrupt running turn execution / cancel current input",
        category: "Global",
        action: KeyAction::EmergencyInterrupt,
    },
    Keybinding {
        key: "Ctrl+Q",
        description: "Exit M31A interactive terminal session",
        category: "Global",
        action: KeyAction::Quit,
    },
    Keybinding {
        key: "Ctrl+P",
        description: "Open universal fuzzy command palette",
        category: "Global",
        action: KeyAction::CommandPalette,
    },
    Keybinding {
        key: "Ctrl+H",
        description: "Open help dialog and full shortcut reference",
        category: "Global",
        action: KeyAction::Help,
    },
    Keybinding {
        key: "F1",
        description: "Open help dialog (standard function key)",
        category: "Global",
        action: KeyAction::Help,
    },
    Keybinding {
        key: "Escape",
        description: "Dismiss open modal / cancel popup / close palette",
        category: "Navigation",
        action: KeyAction::DismissModal,
    },
    // Level 1: Modals & Modes
    Keybinding {
        key: "Ctrl+M",
        description: "Open NVIDIA NIM / universal model selection palette",
        category: "Model",
        action: KeyAction::ModelPicker,
    },
    Keybinding {
        key: "Alt+M",
        description: "Open model selection palette (legacy terminal fallback)",
        category: "Model",
        action: KeyAction::ModelPicker,
    },
    Keybinding {
        key: "F2",
        description: "Open model selection palette (unambiguous function key)",
        category: "Model",
        action: KeyAction::ModelPicker,
    },
    Keybinding {
        key: "Ctrl+S",
        description: "Open interactive settings & configuration manager",
        category: "Configuration",
        action: KeyAction::Settings,
    },
    Keybinding {
        key: "Ctrl+D",
        description: "View uncommitted Git diff in working directory",
        category: "Git",
        action: KeyAction::GitDiff,
    },
    Keybinding {
        key: "Ctrl+Y",
        description: "Copy latest assistant code block to system clipboard",
        category: "Clipboard",
        action: KeyAction::CopyCodeBlock,
    },
    Keybinding {
        key: "Ctrl+W",
        description: "List and manage isolated Git worktrees",
        category: "Git",
        action: KeyAction::Worktrees,
    },
    Keybinding {
        key: "Ctrl+R",
        description: "Open session picker to resume historical conversation",
        category: "Session",
        action: KeyAction::ResumeSession,
    },
    Keybinding {
        key: "Ctrl+N",
        description: "Start a clean development workspace session",
        category: "Session",
        action: KeyAction::NewSession,
    },
    Keybinding {
        key: "Ctrl+B",
        description: "Toggle contextual workspace panel",
        category: "View",
        action: KeyAction::ToggleInspector,
    },
    Keybinding {
        key: "F12",
        description: "Toggle contextual workspace panel (function key)",
        category: "View",
        action: KeyAction::ToggleInspector,
    },
    Keybinding {
        key: "Ctrl+A",
        description: "Show session token usage & cost analytics dashboard",
        category: "Diagnostics",
        action: KeyAction::Analytics,
    },
    Keybinding {
        key: "Ctrl+B",
        description: "Cycle sandbox policy (ReadOnly -> WorkspaceWrite -> DangerFullAccess)",
        category: "Security",
        action: KeyAction::CycleSandboxMode,
    },
    Keybinding {
        key: "Tab",
        description: "Toggle Plan mode vs Default autonomous Build execution",
        category: "Execution",
        action: KeyAction::TogglePlanMode,
    },
    Keybinding {
        key: "Ctrl+T",
        description: "Toggle Plan mode vs Default autonomous Build execution (alias)",
        category: "Execution",
        action: KeyAction::TogglePlanMode,
    },
    Keybinding {
        key: "Ctrl+O",
        description: "Open interactive V8 Code Mode TypeScript/JS scratchpad",
        category: "Execution",
        action: KeyAction::CodeModeScratchpad,
    },
    Keybinding {
        key: "F5",
        description: "Execute active script in Code Mode scratchpad",
        category: "Execution",
        action: KeyAction::CodeModeScratchpad,
    },
    Keybinding {
        key: "Ctrl+K",
        description: "Display causal critical path & tool execution timings modal",
        category: "Diagnostics",
        action: KeyAction::TimingDashboard,
    },
    Keybinding {
        key: "Ctrl+E",
        description: "Inspect active path reader-writer concurrency locks & leases",
        category: "Concurrency",
        action: KeyAction::ConcurrencyLocks,
    },
    Keybinding {
        key: "Ctrl+Z",
        description: "Revert session and workspace changes to the previous checkpoint",
        category: "Session",
        action: KeyAction::UndoCheckpoint,
    },
    Keybinding {
        key: "Ctrl+L",
        description: "Clear the active conversation transcript view",
        category: "Session",
        action: KeyAction::ClearTranscript,
    },
    // Level 2: Subagent Tab Navigation
    Keybinding {
        key: "Alt+Right",
        description: "Switch to next subagent session tab",
        category: "Multi-Agent",
        action: KeyAction::SubagentNextTab,
    },
    Keybinding {
        key: "Alt+Left",
        description: "Switch to previous subagent session tab",
        category: "Multi-Agent",
        action: KeyAction::SubagentPrevTab,
    },
    Keybinding {
        key: "Alt+1..9",
        description: "Jump directly to subagent session tab index",
        category: "Multi-Agent",
        action: KeyAction::SubagentSelectTab(0),
    },
    // Level 3: Composer & Prompt Submission
    Keybinding {
        key: "Enter",
        description: "Submit prompt in single-line composer / select modal item",
        category: "Composer",
        action: KeyAction::SubmitPrompt,
    },
    Keybinding {
        key: "Ctrl+Enter",
        description: "Queue prompt for execution / execute scratchpad script",
        category: "Composer",
        action: KeyAction::QueuePrompt,
    },
    Keybinding {
        key: "Shift+Enter",
        description: "Insert newline without submitting prompt",
        category: "Composer",
        action: KeyAction::InsertNewline,
    },
    Keybinding {
        key: "Alt+Enter",
        description: "Insert newline without submitting prompt (terminal fallback)",
        category: "Composer",
        action: KeyAction::InsertNewline,
    },
    Keybinding {
        key: "PageUp",
        description: "Scroll transcript view up one page",
        category: "Navigation",
        action: KeyAction::PageUp,
    },
    Keybinding {
        key: "PageDown",
        description: "Scroll transcript view down one page",
        category: "Navigation",
        action: KeyAction::PageDown,
    },
];

pub fn canonical_keybindings() -> &'static [Keybinding] {
    CANONICAL_KEYBINDINGS
}

pub fn keybindings_by_category(category: &str) -> Vec<&'static Keybinding> {
    CANONICAL_KEYBINDINGS
        .iter()
        .filter(|kb| kb.category.eq_ignore_ascii_case(category))
        .collect()
}

pub fn render_keybindings_table() -> String {
    let mut out = String::from("| Shortcut | Category | Description |\n|---|---|---|\n");
    for kb in CANONICAL_KEYBINDINGS {
        out.push_str(&format!(
            "| `{}` | {} | {} |\n",
            kb.key, kb.category, kb.description
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_keybindings_completeness() {
        assert!(CANONICAL_KEYBINDINGS.len() >= 20);
        let ctrl_c = CANONICAL_KEYBINDINGS.iter().find(|k| k.key == "Ctrl+C");
        assert!(ctrl_c.is_some());
        assert_eq!(ctrl_c.unwrap().action, KeyAction::EmergencyInterrupt);

        let ctrl_enter = CANONICAL_KEYBINDINGS.iter().find(|k| k.key == "Ctrl+Enter");
        assert!(ctrl_enter.is_some());
        assert_eq!(ctrl_enter.unwrap().action, KeyAction::QueuePrompt);

        let f2 = CANONICAL_KEYBINDINGS.iter().find(|k| k.key == "F2");
        assert!(f2.is_some());
        assert_eq!(f2.unwrap().action, KeyAction::ModelPicker);

        let ctrl_d = CANONICAL_KEYBINDINGS.iter().find(|k| k.key == "Ctrl+D");
        assert!(ctrl_d.is_some());
        assert_eq!(ctrl_d.unwrap().action, KeyAction::GitDiff);

        let ctrl_y = CANONICAL_KEYBINDINGS.iter().find(|k| k.key == "Ctrl+Y");
        assert!(ctrl_y.is_some());
        assert_eq!(ctrl_y.unwrap().action, KeyAction::CopyCodeBlock);

        let ctrl_q = CANONICAL_KEYBINDINGS.iter().find(|k| k.key == "Ctrl+Q");
        assert!(ctrl_q.is_some());
        assert_eq!(ctrl_q.unwrap().action, KeyAction::Quit);

        let ctrl_p = CANONICAL_KEYBINDINGS.iter().find(|k| k.key == "Ctrl+P");
        assert!(ctrl_p.is_some());
        assert_eq!(ctrl_p.unwrap().action, KeyAction::CommandPalette);

        let ctrl_w = CANONICAL_KEYBINDINGS.iter().find(|k| k.key == "Ctrl+W");
        assert!(ctrl_w.is_some());
        assert_eq!(ctrl_w.unwrap().action, KeyAction::Worktrees);

        let ctrl_r = CANONICAL_KEYBINDINGS.iter().find(|k| k.key == "Ctrl+R");
        assert!(ctrl_r.is_some());
        assert_eq!(ctrl_r.unwrap().action, KeyAction::ResumeSession);

        let ctrl_a = CANONICAL_KEYBINDINGS.iter().find(|k| k.key == "Ctrl+A");
        assert!(ctrl_a.is_some());
        assert_eq!(ctrl_a.unwrap().action, KeyAction::Analytics);
    }
}
