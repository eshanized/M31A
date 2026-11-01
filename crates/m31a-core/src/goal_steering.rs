//! Goal steering and token accounting engine.
//!
//! Dynamic goal steering and progress accounting keeping agent actions aligned with active milestones:
//! - Formulates prompt steering fragments for active goals.
//! - Enforces token and time budget limits.
//! - Injects continuation directives and completion guidance.
//! - Manages goal status transitions.

use serde::{Deserialize, Serialize};

/// Budget evaluation state for an active goal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalBudgetStatus {
    WithinBudget,
    ApproachingLimit { percent_used: u64 },
    Exceeded { overflow_tokens: u64 },
    Unbounded,
}

/// Represents an active or tracked goal objective with budget accounting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GoalObjective {
    pub thread_id: String,
    pub objective: String,
    pub status: String,
    pub tokens_used: u64,
    pub token_budget: Option<u64>,
    pub created_at_epoch_secs: u64,
    pub update_plan_enabled: bool,
}

impl GoalObjective {
    pub fn new(
        thread_id: impl Into<String>,
        objective: impl Into<String>,
        token_budget: Option<u64>,
    ) -> Self {
        Self {
            thread_id: thread_id.into(),
            objective: objective.into(),
            status: "active".to_string(),
            tokens_used: 0,
            token_budget,
            created_at_epoch_secs: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            update_plan_enabled: true,
        }
    }

    pub fn render_continuation(&self) -> String {
        render_goal_continuation(
            &self.objective,
            &self.status,
            self.tokens_used,
            self.token_budget,
            self.update_plan_enabled,
        )
    }

    pub fn evaluate_budget(&self) -> GoalBudgetStatus {
        evaluate_goal_budget(self.tokens_used, self.token_budget)
    }
}

/// Helper for rendering XML-safe text within prompt directives.
pub fn escape_xml_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Evaluates token budget status for a goal given current token metrics.
pub fn evaluate_goal_budget(tokens_used: u64, token_budget: Option<u64>) -> GoalBudgetStatus {
    match token_budget {
        Some(budget) if budget > 0 => {
            if tokens_used >= budget {
                GoalBudgetStatus::Exceeded {
                    overflow_tokens: tokens_used - budget,
                }
            } else {
                let percent = (tokens_used * 100) / budget;
                if percent >= 80 {
                    GoalBudgetStatus::ApproachingLimit {
                        percent_used: percent,
                    }
                } else {
                    GoalBudgetStatus::WithinBudget
                }
            }
        }
        _ => GoalBudgetStatus::Unbounded,
    }
}

/// Renders a continuation steering directive injected into the model's turn context.
pub fn render_goal_continuation(
    objective: &str,
    status: &str,
    tokens_used: u64,
    token_budget: Option<u64>,
    update_plan_enabled: bool,
) -> String {
    let safe_obj = escape_xml_text(objective);
    let budget_str = token_budget
        .map(|b| b.to_string())
        .unwrap_or_else(|| "unbounded".to_string());

    let remaining_str = token_budget
        .map(|b| (b.saturating_sub(tokens_used)).to_string())
        .unwrap_or_else(|| "unbounded".to_string());

    let mut prompt = format!(
        "<goal_steering>\n\
         Active Mission Goal: \"{}\"\n\
         Status: {}\n\
         Token Accounting: {} used / {} budget ({} remaining)\n\n\
         Instructions:\n\
         1. Continue working autonomously toward the goal objective without stopping prematurely.\n\
         2. Verify intermediate deliverables using test and build tools.\n",
        safe_obj, status, tokens_used, budget_str, remaining_str
    );

    if update_plan_enabled {
        prompt.push_str(
            "3. Keep your plan updated using the `update_plan` tool as steps complete.\n",
        );
    } else {
        prompt.push_str("3. Log clear progress checkpoints after each major subtask.\n");
    }

    prompt.push_str(
        "4. When all criteria for the objective are satisfied, mark the goal complete and provide a final summary.\n\
         </goal_steering>"
    );

    prompt
}

/// Renders a budget limit warning directive when token usage nears or exceeds allocation.
pub fn render_goal_budget_warning(
    objective: &str,
    tokens_used: u64,
    token_budget: u64,
    time_used_seconds: u64,
) -> String {
    let safe_obj = escape_xml_text(objective);
    format!(
        "<goal_budget_alert>\n\
         CRITICAL NOTICE: Token budget for goal \"{}\" has reached or exceeded limits.\n\
         Usage: {} tokens used against {} budget limit (Elapsed: {}s).\n\n\
         Directive:\n\
         1. Cease exploratory tasks or broad refactorings immediately.\n\
         2. Execute final test verifications on code changes implemented so far.\n\
         3. Prepare a concise completion summary detailing delivered features and remaining backlog.\n\
         4. Conclude the session gracefully.\n\
         </goal_budget_alert>",
        safe_obj, tokens_used, token_budget, time_used_seconds
    )
}

/// Renders an objective updated notification when the user redirects the goal mid-session.
pub fn render_goal_objective_updated(previous_objective: &str, new_objective: &str) -> String {
    let safe_prev = escape_xml_text(previous_objective);
    let safe_new = escape_xml_text(new_objective);
    format!(
        "<goal_redirected>\n\
         ATTENTION: The user has updated the active session goal.\n\
         Previous Objective: \"{}\"\n\
         New Objective:      \"{}\"\n\n\
         Please realign your execution strategy and task plan to satisfy the new objective.\n\
         </goal_redirected>",
        safe_prev, safe_new
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_xml_text() {
        assert_eq!(
            escape_xml_text("Fix & test <User> \"Auth\" 'tokens'"),
            "Fix &amp; test &lt;User&gt; &quot;Auth&quot; &apos;tokens&apos;"
        );
    }

    #[test]
    fn test_evaluate_goal_budget() {
        assert_eq!(
            evaluate_goal_budget(500, Some(1000)),
            GoalBudgetStatus::WithinBudget
        );

        assert_eq!(
            evaluate_goal_budget(850, Some(1000)),
            GoalBudgetStatus::ApproachingLimit { percent_used: 85 }
        );

        assert_eq!(
            evaluate_goal_budget(1050, Some(1000)),
            GoalBudgetStatus::Exceeded {
                overflow_tokens: 50
            }
        );

        assert_eq!(
            evaluate_goal_budget(5000, None),
            GoalBudgetStatus::Unbounded
        );
    }

    #[test]
    fn test_render_goal_continuation() {
        let text = render_goal_continuation(
            "Implement high-performance AST parser",
            "active",
            2500,
            Some(10000),
            true,
        );
        assert!(text.contains("<goal_steering>"));
        assert!(text.contains("Implement high-performance AST parser"));
        assert!(text.contains("2500 used / 10000 budget (7500 remaining)"));
        assert!(text.contains("update_plan"));
    }

    #[test]
    fn test_render_goal_budget_warning() {
        let warning = render_goal_budget_warning("Build AST", 10200, 10000, 120);
        assert!(warning.contains("<goal_budget_alert>"));
        assert!(warning.contains("10200 tokens used against 10000 budget limit"));
        assert!(warning.contains("Cease exploratory tasks"));
    }

    #[test]
    fn test_render_goal_objective_updated() {
        let updated = render_goal_objective_updated("Old task", "New task");
        assert!(updated.contains("<goal_redirected>"));
        assert!(updated.contains("Old task"));
        assert!(updated.contains("New task"));
    }
}
