//! Collaboration session modes and access templates.
//!
//! Defines session profiles (Pair Programming, Architect, Code Review, Guided Discussion) and permissions.

use serde::{Deserialize, Serialize};

/// Supported collaboration modes governing agent permissions, tool availability, and prompt style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum CollaborationModeKind {
    #[default]
    Default,
    Plan,
    Review,
    Debugger,
}

/// Dynamic contextual parameters interpolated into mode templates.
#[derive(Debug, Clone, Default)]
pub struct CollaborationContext {
    pub workspace_dir: String,
    pub model_name: String,
    pub active_role: Option<String>,
    pub git_branch: Option<String>,
}

/// Engine that renders system instructions and enforces mode rules.
pub struct CollaborationTemplateEngine;

impl CollaborationTemplateEngine {
    /// Renders the mode-specific template prompt with context variable substitution.
    pub fn render(mode: CollaborationModeKind, ctx: &CollaborationContext) -> String {
        match mode {
            CollaborationModeKind::Default => Self::render_default_mode(ctx),
            CollaborationModeKind::Plan => Self::render_plan_mode(ctx),
            CollaborationModeKind::Review => Self::render_review_mode(ctx),
            CollaborationModeKind::Debugger => Self::render_debugger_mode(ctx),
        }
    }

    /// Verifies whether an action or tool is permissible under the active collaboration mode.
    /// In `PlanMode`, mutating actions (writing files, applying patches) are strictly rejected.
    pub fn is_tool_allowed(mode: CollaborationModeKind, tool_name: &str) -> bool {
        match mode {
            CollaborationModeKind::Default | CollaborationModeKind::Debugger => true,
            CollaborationModeKind::Plan => {
                // Strictly non-mutating in plan mode
                let mutating_tools = ["write_file", "apply_patch", "update_plan"];
                !mutating_tools.contains(&tool_name)
            }
            CollaborationModeKind::Review => {
                // Read-only inspection in review mode
                let mutating_tools = ["write_file", "apply_patch"];
                !mutating_tools.contains(&tool_name)
            }
        }
    }

    /// Extracts `<proposed_plan>...</proposed_plan>` XML blocks from a completed plan turn.
    pub fn extract_proposed_plan(response: &str) -> Option<String> {
        let open_tag = "<proposed_plan>";
        let close_tag = "</proposed_plan>";
        if let (Some(start), Some(end)) = (response.find(open_tag), response.rfind(close_tag))
            && start + open_tag.len() < end
        {
            let plan_content = response[start + open_tag.len()..end].trim();
            return Some(plan_content.to_string());
        }
        None
    }

    fn render_default_mode(ctx: &CollaborationContext) -> String {
        format!(
            "# Collaboration Mode: Default\n\n\
You are now in Default autonomous mode in workspace `{}`.\n\
- Strongly prefer making reasonable, high-confidence assumptions and taking direct action rather than stopping to ask clarifying questions.\n\
- Use `request_user_input` only for high-leverage optional decisions where user input materially impacts outcomes.\n\
- Keep execution momentum: explore, edit, test, and verify cleanly.",
            ctx.workspace_dir
        )
    }

    fn render_plan_mode(ctx: &CollaborationContext) -> String {
        format!(
            "# Collaboration Mode: Plan (Conversational)\n\n\
You are in Plan Mode in workspace `{}`. You operate in 3 structured phases to produce a decision-complete plan:\n\n\
## Mode Rules (Strict)\n\
- You remain in Plan Mode until a developer instruction explicitly transitions out of it.\n\
- Mutating tools (`write_file`, `apply_patch`, `update_plan`) are forbidden in Plan Mode.\n\
- Non-mutating truth-seeking actions (`read_file`, `file_search`, `grep_search`, `list_dir`) are strongly encouraged.\n\n\
## Phase 1 — Environment Grounding\n\
Explore the codebase first. Discover file paths, types, interfaces, and architecture before asking questions.\n\n\
## Phase 2 — Intent Alignment\n\
Clarify goals, boundaries, scope, audience, and architectural trade-offs.\n\n\
## Phase 3 — Decision-Complete Specification\n\
Detail the precise files to modify, new types, API contracts, failure modes, and test plans.\n\
When finalized, wrap your complete blueprint inside:\n\
<proposed_plan>\n\
### Plan Summary\n\
...\n\
</proposed_plan>",
            ctx.workspace_dir
        )
    }

    fn render_review_mode(ctx: &CollaborationContext) -> String {
        format!(
            "# Collaboration Mode: Code Review\n\n\
You are in Code Review mode in workspace `{}`.\n\
- Inspect uncommitted changes, turn diffs, and recent commits.\n\
- Organize findings into 3 explicit tiers: [Blocker], [Warning], and [Suggestion].\n\
- Emphasize correctness, performance regressions, security vulnerabilities, edge-case coverage, and documentation integrity.",
            ctx.workspace_dir
        )
    }

    fn render_debugger_mode(ctx: &CollaborationContext) -> String {
        format!(
            "# Collaboration Mode: Debugger\n\n\
You are in Debugger mode in workspace `{}`.\n\
- Step 1: Reproduce the error with a minimal failing test or reproduction command.\n\
- Step 2: Trace the root cause through inspectable logs and backtraces.\n\
- Step 3: Implement the most surgical, minimal fix that cures the root cause without collateral changes.\n\
- Step 4: Re-run tests to confirm regression immunity.",
            ctx.workspace_dir
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collaboration_templates_rendering() {
        let ctx = CollaborationContext {
            workspace_dir: "/home/user/project".to_string(),
            model_name: "meta/llama-3.3-70b-instruct".to_string(),
            active_role: Some("architect".to_string()),
            git_branch: Some("feat/perf".to_string()),
        };

        let def = CollaborationTemplateEngine::render(CollaborationModeKind::Default, &ctx);
        assert!(def.contains("Default autonomous mode"));
        assert!(def.contains("/home/user/project"));

        let plan = CollaborationTemplateEngine::render(CollaborationModeKind::Plan, &ctx);
        assert!(plan.contains("Plan (Conversational)"));
        assert!(plan.contains("<proposed_plan>"));

        let review = CollaborationTemplateEngine::render(CollaborationModeKind::Review, &ctx);
        assert!(review.contains("Code Review mode"));
        assert!(review.contains("[Blocker]"));

        let debug = CollaborationTemplateEngine::render(CollaborationModeKind::Debugger, &ctx);
        assert!(debug.contains("Debugger mode"));
        assert!(debug.contains("Reproduce the error"));
    }

    #[test]
    fn test_plan_mode_mutating_tool_restriction() {
        assert!(!CollaborationTemplateEngine::is_tool_allowed(
            CollaborationModeKind::Plan,
            "write_file"
        ));
        assert!(!CollaborationTemplateEngine::is_tool_allowed(
            CollaborationModeKind::Plan,
            "apply_patch"
        ));
        assert!(!CollaborationTemplateEngine::is_tool_allowed(
            CollaborationModeKind::Plan,
            "update_plan"
        ));
        assert!(CollaborationTemplateEngine::is_tool_allowed(
            CollaborationModeKind::Plan,
            "read_file"
        ));
        assert!(CollaborationTemplateEngine::is_tool_allowed(
            CollaborationModeKind::Plan,
            "file_search"
        ));

        assert!(CollaborationTemplateEngine::is_tool_allowed(
            CollaborationModeKind::Default,
            "write_file"
        ));
    }

    #[test]
    fn test_extract_proposed_plan() {
        let text = "Here is my thought process.\n<proposed_plan>\n### Architecture Migration\n1. Add new table\n2. Update ORM\n</proposed_plan>\nPlease approve.";
        let plan = CollaborationTemplateEngine::extract_proposed_plan(text).unwrap();
        assert!(plan.contains("### Architecture Migration"));
        assert!(plan.contains("Update ORM"));
    }
}
