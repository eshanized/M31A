//! Strongly typed interactive decision model for user choices and workflow approvals (PRD §01, CLI-01).
//!
//! Provides a unified, structured interaction abstraction replacing command-only prompts
//! (such as `/plan accept`, `/tasks accept`, `/no-plan`, etc.) with native, interactive
//! decision points across both CLI and TUI interfaces.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Specific domain kind of interactive decision being resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionKind {
    /// Planning mode selection (project-level planning docs vs. direct execution).
    PlanningChoice,

    /// Candidate plan review.
    PlanReview {
        plan_revision: u32,
        #[serde(default)]
        content_hash: Option<String>,
    },

    /// Candidate task set review.
    TaskReview {
        plan_revision: u32,
        task_revision: u32,
        #[serde(default)]
        content_hash: Option<String>,
    },

    /// Workspace-modification execution launch authorization.
    ExecutionAuthorization {
        plan_revision: u32,
        task_revision: u32,
    },

    /// Clarification question from intent discovery or refinement.
    Clarification {
        question_id: String,
        allow_freeform: bool,
    },

    /// Tool invocation policy permission approval.
    PermissionApproval {
        tool_name: String,
        risk_tier: String,
    },

    /// Architecture or implementation strategy selection.
    ImplementationStrategy,

    /// Recovery strategy choice after a task or verification failure.
    RecoveryChoice,
}

/// A selectable option presented to the operator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionOption {
    /// Stable internal identifier for this option (e.g. "accept", "reject", "revise", "plan", "direct").
    pub id: String,

    /// Short human-readable display label (e.g. "Accept plan and continue").
    pub label: String,

    /// Concise description of what selecting this option will do.
    pub description: String,
}

impl DecisionOption {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: description.into(),
        }
    }
}

/// Lifecycle status of an interactive decision request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionState {
    /// Waiting for operator response.
    Pending,

    /// Operator selected an option or submitted an answer.
    Answered {
        selected_option_id: Option<String>,
        #[serde(default)]
        selected_option_ids: Vec<String>,
        custom_answer: Option<String>,
    },

    /// Operator explicitly rejected this request.
    Rejected { reason: Option<String> },

    /// Operator cancelled or dismissed this decision.
    Cancelled,

    /// Decision timed out or was superseded by a newer revision.
    Expired,
}

impl DecisionState {
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    pub fn is_terminal(&self) -> bool {
        !self.is_pending()
    }
}

/// Unified interactive decision model.
///
/// Encapsulates all metadata necessary to render an interactive question or choice prompt
/// in the TUI or CLI, and maps operator selections back to unambiguous domain transitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveDecision {
    /// Stable unique identifier for this pending request.
    pub id: String,

    /// Owning session identifier.
    pub session_id: String,

    /// Optional execution or mission identifier associated with this decision.
    pub execution_id: Option<String>,

    /// Short header or title (e.g. "Plan Review", "Project Planning Decision").
    pub title: String,

    /// Natural-language prompt or question text.
    pub prompt: String,

    /// Selectable options.
    pub options: Vec<DecisionOption>,

    /// Currently focused or selected option index (0-indexed).
    pub selected_index: usize,

    /// Whether multiple choices can be selected simultaneously.
    pub is_multi_select: bool,

    /// Whether free-form custom text input is permitted.
    pub allow_custom_answer: bool,

    /// Custom text entered by operator, if any.
    pub custom_answer: Option<String>,

    /// Specific decision kind.
    pub kind: DecisionKind,

    /// Lifecycle status.
    pub state: DecisionState,

    /// Timestamp when this decision was created.
    pub created_at: DateTime<Utc>,
}

impl InteractiveDecision {
    /// Create a new interactive decision for project planning mode choice.
    pub fn planning_choice(
        session_id: impl Into<String>,
        _raw_prompt: impl Into<String>,
        question: impl Into<String>,
    ) -> Self {
        let sid = session_id.into();
        Self {
            id: format!("planning_choice_{sid}"),
            session_id: sid,
            execution_id: None,
            title: "Project Planning Decision".to_string(),
            prompt: question.into(),
            options: vec![
                DecisionOption::new(
                    "plan",
                    "Create a plan first",
                    "Synthesize project-level planning documents under .m31a/project/ (Genesis)",
                ),
                DecisionOption::new(
                    "direct",
                    "Implement directly",
                    "Work directly on this request without project-level planning documents",
                ),
                DecisionOption::new(
                    "cancel",
                    "Cancel request",
                    "Cancel this pending operation and return to ready state",
                ),
            ],
            selected_index: 0,
            is_multi_select: false,
            allow_custom_answer: false,
            custom_answer: None,
            kind: DecisionKind::PlanningChoice,
            state: DecisionState::Pending,
            created_at: Utc::now(),
        }
    }

    /// Create a new interactive decision for candidate plan review.
    pub fn plan_review(
        session_id: impl Into<String>,
        revision: u32,
        objective: impl Into<String>,
        task_count: usize,
        content_hash: Option<String>,
    ) -> Self {
        let sid = session_id.into();
        let obj = objective.into();
        Self {
            id: format!("plan_review_{sid}_{revision}"),
            session_id: sid,
            execution_id: None,
            title: format!("Plan Review · Revision {revision}"),
            prompt: format!("Proposed {task_count} tasks for '{obj}'. How should M31A proceed?"),
            options: vec![
                DecisionOption::new(
                    "accept",
                    "Accept plan and continue",
                    "Approve this candidate plan and advance to task graph breakdown",
                ),
                DecisionOption::new(
                    "revise",
                    "Request revision",
                    "Supply feedback to have the model revise the candidate plan",
                ),
                DecisionOption::new(
                    "regen",
                    "Regenerate plan",
                    "Discard candidate plan and generate an alternate plan",
                ),
                DecisionOption::new(
                    "reject",
                    "Reject plan",
                    "Reject the proposed plan and halt pre-execution",
                ),
            ],
            selected_index: 0,
            is_multi_select: false,
            allow_custom_answer: true,
            custom_answer: None,
            kind: DecisionKind::PlanReview {
                plan_revision: revision,
                content_hash,
            },
            state: DecisionState::Pending,
            created_at: Utc::now(),
        }
    }

    /// Create a new interactive decision for candidate task set review.
    pub fn tasks_review(
        session_id: impl Into<String>,
        plan_revision: u32,
        task_revision: u32,
        task_count: usize,
        content_hash: Option<String>,
    ) -> Self {
        let sid = session_id.into();
        Self {
            id: format!("tasks_review_{sid}_{task_revision}"),
            session_id: sid,
            execution_id: None,
            title: format!("Task Breakdown Review · Revision {task_revision}"),
            prompt: format!("Candidate task set contains {task_count} tasks. Choose an action:"),
            options: vec![
                DecisionOption::new(
                    "accept",
                    "Accept tasks and proceed",
                    "Approve candidate tasks and request execution authorization",
                ),
                DecisionOption::new(
                    "regen",
                    "Regenerate task graph",
                    "Request model to regenerate the task set with optional notes",
                ),
                DecisionOption::new(
                    "reject",
                    "Reject tasks",
                    "Reject candidate tasks and return to planning",
                ),
            ],
            selected_index: 0,
            is_multi_select: false,
            allow_custom_answer: true,
            custom_answer: None,
            kind: DecisionKind::TaskReview {
                plan_revision,
                task_revision,
                content_hash,
            },
            state: DecisionState::Pending,
            created_at: Utc::now(),
        }
    }

    /// Create a new interactive decision for execution authorization.
    pub fn execution_authorization(
        session_id: impl Into<String>,
        plan_revision: u32,
        task_revision: u32,
        message: impl Into<String>,
    ) -> Self {
        let sid = session_id.into();
        Self {
            id: format!("auth_{sid}_{plan_revision}_{task_revision}"),
            session_id: sid,
            execution_id: None,
            title: "Execution Authorization Required".to_string(),
            prompt: message.into(),
            options: vec![
                DecisionOption::new(
                    "authorize",
                    "Authorize execution",
                    "Authorize workspace modifications and begin executing approved tasks",
                ),
                DecisionOption::new(
                    "reject",
                    "Reject authorization",
                    "Deny authorization and halt without modifying workspace",
                ),
            ],
            selected_index: 0,
            is_multi_select: false,
            allow_custom_answer: false,
            custom_answer: None,
            kind: DecisionKind::ExecutionAuthorization {
                plan_revision,
                task_revision,
            },
            state: DecisionState::Pending,
            created_at: Utc::now(),
        }
    }

    /// Create an interactive decision from a discovery question.
    pub fn discovery_question(
        session_id: impl Into<String>,
        question: &crate::workflow::genesis::DynamicQuestion,
    ) -> Self {
        let sid = session_id.into();
        let options: Vec<DecisionOption> = question
            .options
            .iter()
            .enumerate()
            .map(|(idx, opt)| {
                DecisionOption::new(format!("opt_{}", idx + 1), opt.clone(), "Predefined choice")
            })
            .collect();

        Self {
            id: question.question_id.clone(),
            session_id: sid,
            execution_id: None,
            title: format!("Clarification · {}", question.target_unknown),
            prompt: question.text.clone(),
            options,
            selected_index: 0,
            is_multi_select: false,
            allow_custom_answer: question.allow_freeform,
            custom_answer: None,
            kind: DecisionKind::Clarification {
                question_id: question.question_id.clone(),
                allow_freeform: question.allow_freeform,
            },
            state: DecisionState::Pending,
            created_at: Utc::now(),
        }
    }

    /// Create an interactive decision for tool permission approval.
    pub fn permission_approval(
        session_id: impl Into<String>,
        request_id: impl Into<String>,
        tool_name: impl Into<String>,
        details: impl Into<String>,
        risk_tier: impl Into<String>,
        parameters_summary: Option<String>,
    ) -> Self {
        let sid = session_id.into();
        let req_id = request_id.into();
        let t_name = tool_name.into();
        let r_tier = risk_tier.into();
        let prompt_text = if let Some(p) = parameters_summary {
            format!("{}: {}", details.into(), p)
        } else {
            details.into()
        };

        Self {
            id: req_id,
            session_id: sid,
            execution_id: None,
            title: format!("Approval Required · {} ({})", t_name, r_tier),
            prompt: prompt_text,
            options: vec![
                DecisionOption::new(
                    "approve_once",
                    "Approve once",
                    "Allow this single execution of the guarded operation",
                ),
                DecisionOption::new(
                    "approve_always",
                    "Approve for session",
                    "Allow this tool without prompting for the duration of this session",
                ),
                DecisionOption::new(
                    "reject",
                    "Reject",
                    "Deny execution and block this tool invocation",
                ),
            ],
            selected_index: 0,
            is_multi_select: false,
            allow_custom_answer: false,
            custom_answer: None,
            kind: DecisionKind::PermissionApproval {
                tool_name: t_name,
                risk_tier: r_tier,
            },
            state: DecisionState::Pending,
            created_at: Utc::now(),
        }
    }

    /// Move focus to next selectable option.
    pub fn select_next(&mut self) {
        if !self.options.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.options.len();
        }
    }

    /// Move focus to previous selectable option.
    pub fn select_prev(&mut self) {
        if !self.options.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.options.len() - 1
            } else {
                self.selected_index - 1
            };
        }
    }

    /// Explicitly select an option by index if valid.
    pub fn select_index(&mut self, idx: usize) -> bool {
        if idx < self.options.len() {
            self.selected_index = idx;
            true
        } else {
            false
        }
    }

    /// Return reference to the currently focused option.
    pub fn selected_option(&self) -> Option<&DecisionOption> {
        self.options.get(self.selected_index)
    }

    /// Set freeform custom answer text.
    pub fn set_custom_answer(&mut self, text: impl Into<String>) {
        self.custom_answer = Some(text.into());
    }

    /// Validate a proposed response against this pending decision.
    pub fn validate_response(&self, resp: &InteractiveDecisionResponse) -> Result<(), String> {
        if resp.decision_id != self.id {
            return Err(format!(
                "Decision ID mismatch: expected '{}', received '{}'",
                self.id, resp.decision_id
            ));
        }

        if resp.session_id != self.session_id {
            return Err(format!(
                "Session ID mismatch: expected '{}', received '{}'",
                self.session_id, resp.session_id
            ));
        }

        if !self.state.is_pending() {
            return Err(format!(
                "Decision '{}' is no longer pending (current state: {:?})",
                self.id, self.state
            ));
        }

        if resp.cancelled {
            return Ok(());
        }

        if let Some(ref opt_id) = resp.selected_option_id {
            if !self.options.iter().any(|o| &o.id == opt_id) {
                return Err(format!(
                    "Invalid option '{}' for decision '{}'. Permitted options: {:?}",
                    opt_id,
                    self.id,
                    self.options.iter().map(|o| &o.id).collect::<Vec<_>>()
                ));
            }
        } else if !resp.selected_option_ids.is_empty() {
            for opt_id in &resp.selected_option_ids {
                if !self.options.iter().any(|o| &o.id == opt_id) {
                    return Err(format!(
                        "Invalid option '{}' for decision '{}'",
                        opt_id, self.id
                    ));
                }
            }
        } else if resp.custom_answer.is_none()
            || resp
                .custom_answer
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            return Err(format!(
                "Decision response for '{}' contains neither a selected option nor custom text",
                self.id
            ));
        }

        Ok(())
    }
}

/// Operator response to an interactive decision request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveDecisionResponse {
    /// Identifier of the decision being answered.
    pub decision_id: String,

    /// Session identifier.
    pub session_id: String,

    /// Associated execution or mission identifier, if any.
    #[serde(default)]
    pub execution_id: Option<String>,

    /// Selected option identifier (for single-select).
    #[serde(default)]
    pub selected_option_id: Option<String>,

    /// Selected option identifiers (for multi-select).
    #[serde(default)]
    pub selected_option_ids: Vec<String>,

    /// Free-text custom answer or feedback, if provided.
    #[serde(default)]
    pub custom_answer: Option<String>,

    /// Whether the operator cancelled or dismissed the request.
    #[serde(default)]
    pub cancelled: bool,
}

impl InteractiveDecisionResponse {
    pub fn select_option(
        decision_id: impl Into<String>,
        session_id: impl Into<String>,
        option_id: impl Into<String>,
    ) -> Self {
        Self {
            decision_id: decision_id.into(),
            session_id: session_id.into(),
            execution_id: None,
            selected_option_id: Some(option_id.into()),
            selected_option_ids: Vec::new(),
            custom_answer: None,
            cancelled: false,
        }
    }

    pub fn with_custom_answer(mut self, text: impl Into<String>) -> Self {
        self.custom_answer = Some(text.into());
        self
    }

    pub fn cancel(decision_id: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            decision_id: decision_id.into(),
            session_id: session_id.into(),
            execution_id: None,
            selected_option_id: None,
            selected_option_ids: Vec::new(),
            custom_answer: None,
            cancelled: true,
        }
    }
}
