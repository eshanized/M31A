//! Interactive approval overlay and decision models for high-risk operations.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalKind {
    ExecCommand {
        command: String,
        risk_level: String,
        suggested_pattern: String,
    },
    ApplyPatch {
        file_path: String,
        diff_preview: String,
        lines_added: usize,
        lines_deleted: usize,
    },
    WriteFile {
        file_path: String,
        bytes_count: usize,
    },
    SpawnAgent {
        role: String,
        instructions: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalDecision {
    AllowOnce,
    /// Allow every future operation that targets this file for the rest of the session.
    AllowFile {
        path: String,
    },
    AlwaysAllowPattern(String),
    Deny {
        reason: String,
    },
    EditCommand {
        new_command: String,
    },
}

impl From<bool> for ApprovalDecision {
    fn from(approved: bool) -> Self {
        if approved {
            ApprovalDecision::AllowOnce
        } else {
            ApprovalDecision::Deny {
                reason: "Operation denied by user.".to_string(),
            }
        }
    }
}

impl ApprovalDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(
            self,
            ApprovalDecision::AllowOnce
                | ApprovalDecision::AllowFile { .. }
                | ApprovalDecision::AlwaysAllowPattern(_)
                | ApprovalDecision::EditCommand { .. }
        )
    }

    pub fn denial_feedback(&self) -> Option<&str> {
        match self {
            ApprovalDecision::Deny { reason } => Some(reason.as_str()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApprovalState {
    pub id: String,
    pub kind: ApprovalKind,
    pub selected_option: usize,
    pub edit_buffer: String,
    pub is_editing: bool,
}

impl ApprovalState {
    pub fn new(kind: ApprovalKind) -> Self {
        let initial_edit = match &kind {
            ApprovalKind::ExecCommand { command, .. } => command.clone(),
            _ => String::new(),
        };

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            selected_option: 0,
            edit_buffer: initial_edit,
            is_editing: false,
        }
    }

    pub fn select_next(&mut self) {
        if !self.is_editing && self.selected_option < 3 {
            self.selected_option += 1;
        }
    }

    pub fn select_prev(&mut self) {
        if !self.is_editing && self.selected_option > 0 {
            self.selected_option -= 1;
        }
    }

    pub fn decide(&self) -> ApprovalDecision {
        match self.selected_option {
            0 => ApprovalDecision::AllowOnce,
            1 => {
                let pattern = match &self.kind {
                    ApprovalKind::ExecCommand {
                        suggested_pattern, ..
                    } => suggested_pattern.clone(),
                    ApprovalKind::ApplyPatch { file_path, .. } => format!("patch:{}", file_path),
                    ApprovalKind::WriteFile { file_path, .. } => format!("write:{}", file_path),
                    ApprovalKind::SpawnAgent { role, .. } => format!("agent:{}", role),
                };
                ApprovalDecision::AlwaysAllowPattern(pattern)
            }
            2 => ApprovalDecision::Deny {
                reason: "Operation denied by user.".to_string(),
            },
            3 => ApprovalDecision::EditCommand {
                new_command: self.edit_buffer.clone(),
            },
            _ => ApprovalDecision::Deny {
                reason: "Cancelled".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_decision_pattern() {
        let state = ApprovalState::new(ApprovalKind::ExecCommand {
            command: "cargo build".into(),
            risk_level: "low".into(),
            suggested_pattern: "cargo *".into(),
        });

        let mut s = state;
        s.selected_option = 1;
        match s.decide() {
            ApprovalDecision::AlwaysAllowPattern(p) => assert_eq!(p, "cargo *"),
            _ => panic!("Expected AlwaysAllowPattern"),
        }
    }
}
