//! System prompt assembly, persona templates, and collaboration mode instructions.
//!
//! Builds context-aware system prompts combining base instructions, workspace rules, skills, and mode guidelines.

use crate::skills::SkillLoader;
use crate::world_state::{Placement, WorldState};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CollaborationMode {
    #[default]
    Default,
    Plan,
}

/// Dynamically injected and pinned workspace file or memory context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DynamicContextItem {
    pub path: String,
    pub content: String,
    pub tokens: usize,
}

pub struct SystemPromptBuilder;

impl SystemPromptBuilder {
    pub fn build(cwd: &Path, model: &str, sandbox_mode: &str, mode: CollaborationMode) -> String {
        Self::build_with_user_prompt(cwd, model, sandbox_mode, mode, None)
    }

    pub fn build_with_user_prompt(
        cwd: &Path,
        model: &str,
        sandbox_mode: &str,
        mode: CollaborationMode,
        user_prompt: Option<&str>,
    ) -> String {
        Self::build_with_goal(cwd, model, sandbox_mode, mode, user_prompt, None)
    }

    /// Builds the prompt as a named [`WorldState`] (report §5.3) — callers
    /// that want section diffing use this instead of the joined string.
    pub fn sections(
        cwd: &Path,
        model: &str,
        sandbox_mode: &str,
        mode: CollaborationMode,
        user_prompt: Option<&str>,
        active_goal: Option<&crate::goal_steering::GoalObjective>,
    ) -> WorldState {
        Self::sections_with_dynamic_context(
            cwd,
            model,
            sandbox_mode,
            mode,
            user_prompt,
            active_goal,
            &[],
        )
    }

    /// Builds world state with dynamic context items and live workspace status.
    pub fn sections_with_dynamic_context(
        cwd: &Path,
        model: &str,
        sandbox_mode: &str,
        mode: CollaborationMode,
        user_prompt: Option<&str>,
        active_goal: Option<&crate::goal_steering::GoalObjective>,
        dynamic_context: &[DynamicContextItem],
    ) -> WorldState {
        let mut state = WorldState::new();

        // 1. Identity & Operating Environment
        let mut identity = String::new();
        identity.push_str("# Identity & Core Persona\n");
        identity.push_str(
            "You are M31A, an autonomous software engineering assistant powered by NVIDIA NIM.\n",
        );
        identity.push_str(
            "You operate directly on the user's codebase with complete execution agency.\n\n",
        );

        identity.push_str("## Environment Context\n");
        identity.push_str(&format!("- Working Directory: `{}`\n", cwd.display()));
        identity.push_str(&format!("- Active Model: `{}`\n", model));
        identity.push_str(&format!("- Sandbox Policy: `{}`\n", sandbox_mode));
        identity.push_str(&format!("- Collaboration Mode: `{:?}`\n", mode));
        identity.push_str("- Operating System: Linux\n");

        // Git context
        if let Ok(output) = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(cwd)
            .output()
            && output.status.success()
        {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !branch.is_empty() {
                identity.push_str(&format!("- Git Branch: `{}`\n", branch));
            }
        }

        identity.push('\n');
        state.push_raw("identity", identity, Placement::Prefix);

        // 2. Core Directives & Tool Protocol
        let mut directives = String::new();
        directives.push_str("## Core Directives\n");
        directives.push_str("1. **Inspect before modifying**: Gather ground truth by reading manifests, configs, and source files before editing.\n");
        directives.push_str("2. **Surgical edits with `apply_patch`**: For modifying files, prefer `apply_patch` with the Lark grammar patch format or `write_file`.\n");
        directives.push_str("3. **Execute verification**: After applying edits, run tests, linters, or compilation commands to verify correctness.\n");
        directives.push_str("4. **Sub-Agent Collaboration**: Use `spawn_agent`, `send_input`, `wait_agent`, and `close_agent` to orchestrate parallel tasks. Note that all agents share the same filesystem workspace.\n");
        directives.push_str("5. **Code Mode Execution**: For complex data filtering, AST transformations, or scripts that would produce large stdout, use Code Mode JS isolates (`code_mode_eval`) with session KV storage (`store(k, v)`, `load(k)`).\n");
        directives.push_str("6. **Goal Tracking**: For long-running or multi-stage objectives, maintain task clarity with `create_goal` and `update_goal`.\n\n");
        state.push_raw("directives", directives, Placement::Prefix);

        // 3. Patch Engine Format Specification
        let mut patch_spec = String::new();
        patch_spec.push_str("## Patch Engine Specification (`apply_patch`)\n");
        patch_spec
            .push_str("When calling `apply_patch` with patch text, use this exact Lark grammar:\n");
        patch_spec.push_str("```\n");
        patch_spec.push_str("*** Begin Patch\n");
        patch_spec.push_str("*** Update File: path/to/file.rs\n");
        patch_spec.push_str("@@ fn existing_function() {\n");
        patch_spec.push_str("-    old_code();\n");
        patch_spec.push_str("+    new_code();\n");
        patch_spec.push_str("*** Add File: path/to/new_file.rs\n");
        patch_spec.push_str("+pub fn hello() {}\n");
        patch_spec.push_str("*** Delete File: path/to/obsolete.rs\n");
        patch_spec.push_str("*** End Patch\n");
        patch_spec.push_str("```\n\n");
        state.push_raw("patch_spec", patch_spec, Placement::Prefix);

        // 4. Collaboration Mode Specific Guidance
        let mut collaboration = String::new();
        match mode {
            CollaborationMode::Plan => {
                collaboration.push_str("## Collaboration Mode: Plan (Conversational)\n");
                collaboration.push_str("You are in **Plan Mode**.\n");
                collaboration.push_str("- You work across 3 phases: Ground in Environment -> Intent Chat -> Implementation Chat.\n");
                collaboration.push_str("- You may execute **non-mutating** actions (reading files, searches, running checks that do not edit repo files).\n");
                collaboration.push_str("- You must **NOT** execute mutating actions (writing/editing files, running formatters/linters that rewrite files, applying patches).\n");
                collaboration.push_str("- Use the `request_user_input` tool for decisions that materially change the plan, or ask concise questions directly.\n");
                collaboration.push_str("- When the plan is decision-complete, wrap the finalized plan inside `<proposed_plan>...</proposed_plan>` tags.\n\n");
            }
            CollaborationMode::Default => {
                collaboration.push_str("## Collaboration Mode: Default\n");
                collaboration.push_str(
                    "- Prioritize executing the user's objective decisively and autonomously.\n",
                );
                collaboration.push_str("- Make reasonable, best-practice engineering decisions rather than pausing for trivial questions.\n\n");
            }
        }
        state.push_raw("collaboration", collaboration, Placement::Prefix);

        // 5. Hierarchical User & Project Instructions (AGENTS.override.md, AGENTS.md, CLAUDE.md, instructions.md)
        let instructions = crate::instructions::HierarchicalInstructions::discover(cwd, None);
        if !instructions.entries.is_empty() {
            state.push_raw(
                "instructions",
                instructions.render_attributed_markdown(),
                Placement::Mergeable,
            );
        }

        // 6. Discovered Skills Catalog & Explicit Mentions
        let skills = SkillLoader::discover_skills(cwd);
        let skills_prompt = SkillLoader::render_skills_prompt(&skills);
        if !skills_prompt.is_empty() {
            let mut rendered = skills_prompt;
            rendered.push('\n');
            state.push_raw("skills", rendered, Placement::Mergeable);
        }

        if let Some(user_input) = user_prompt {
            let explicit = SkillLoader::resolve_explicit_mentions(user_input, &skills);
            let explicit_instructions = SkillLoader::render_explicit_instructions(&explicit);
            if !explicit_instructions.is_empty() {
                let mut rendered = explicit_instructions;
                rendered.push('\n');
                state.push_raw("explicit_skills", rendered, Placement::Mergeable);
            }
        }

        // 7. Persistent Memories & Past Learnings
        if let Ok(mem_mgr) = crate::memories::MemoryManager::new() {
            let mem_prompt = mem_mgr.render_memories_prompt();
            if !mem_prompt.is_empty() {
                let mut rendered = mem_prompt;
                rendered.push('\n');
                state.push_raw("memories", rendered, Placement::Mergeable);
            }
        }

        // 8. Active Goal Steering & Objective Context
        if let Some(goal) = active_goal {
            let mut rendered = goal.render_continuation();
            rendered.push('\n');
            state.push_raw("goal", rendered, Placement::Mergeable);
        }

        // 9. Dynamic Working Context (Pinned files, live snippets)
        if !dynamic_context.is_empty() {
            let mut dynamic_sec = String::new();
            dynamic_sec.push_str("## Dynamic Working Context\n");
            dynamic_sec.push_str(
                "The following files and items are actively pinned in working context:\n\n",
            );
            for item in dynamic_context {
                dynamic_sec.push_str(&format!(
                    "### File: `{}` (tokens: ~{})\n",
                    item.path, item.tokens
                ));
                dynamic_sec.push_str("```\n");
                dynamic_sec.push_str(&item.content);
                if !item.content.ends_with('\n') {
                    dynamic_sec.push('\n');
                }
                dynamic_sec.push_str("```\n\n");
            }
            state.push_raw("dynamic_context", dynamic_sec, Placement::Mergeable);
        }

        // 10. Workspace Live Status (Live modified files tracking)
        if let Ok(output) = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(cwd)
            .output()
            && output.status.success()
        {
            let raw_status = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = raw_status
                .lines()
                .filter(|l| !l.trim().is_empty())
                .collect();
            if !lines.is_empty() {
                let mut ws_sec = String::new();
                ws_sec.push_str("## Live Working Tree Status\n");
                ws_sec.push_str(&format!(
                    "There are {} modified or untracked file(s) in working tree:\n",
                    lines.len()
                ));
                for l in lines.iter().take(15) {
                    ws_sec.push_str(&format!("- `{}`\n", l.trim()));
                }
                if lines.len() > 15 {
                    ws_sec.push_str(&format!("- ... and {} more files\n", lines.len() - 15));
                }
                ws_sec.push('\n');
                state.push_raw("workspace_status", ws_sec, Placement::Prefix);
            }
        }

        state
    }

    pub fn build_with_goal(
        cwd: &Path,
        model: &str,
        sandbox_mode: &str,
        mode: CollaborationMode,
        user_prompt: Option<&str>,
        active_goal: Option<&crate::goal_steering::GoalObjective>,
    ) -> String {
        Self::sections(cwd, model, sandbox_mode, mode, user_prompt, active_goal).render()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_system_prompt_builder() {
        let dir = tempdir().unwrap();
        let prompt = SystemPromptBuilder::build(
            dir.path(),
            "meta/llama-3.3-70b-instruct",
            "WorkspaceWrite",
            CollaborationMode::Default,
        );

        assert!(prompt.contains("You are M31A"));
        assert!(prompt.contains("meta/llama-3.3-70b-instruct"));
        assert!(prompt.contains("WorkspaceWrite"));
        assert!(prompt.contains("Patch Engine Specification"));
        assert!(prompt.contains("apply_patch"));
    }

    #[test]
    fn test_system_prompt_plan_mode() {
        let dir = tempdir().unwrap();
        let prompt = SystemPromptBuilder::build(
            dir.path(),
            "deepseek-ai/deepseek-r1",
            "ReadOnly",
            CollaborationMode::Plan,
        );

        assert!(prompt.contains("Collaboration Mode: Plan"));
        assert!(prompt.contains("<proposed_plan>"));
    }

    #[test]
    fn test_system_prompt_agents_md_injection() {
        let dir = tempdir().unwrap();
        let agents_file = dir.path().join("AGENTS.md");
        fs::write(
            &agents_file,
            "# Project Rules\nAlways run cargo clippy before committing.",
        )
        .unwrap();

        let prompt = SystemPromptBuilder::build(
            dir.path(),
            "meta/llama-3.3-70b-instruct",
            "WorkspaceWrite",
            CollaborationMode::Default,
        );

        assert!(prompt.contains("Project Instructions"));
        assert!(prompt.contains("AGENTS.md"));
        assert!(prompt.contains("Always run cargo clippy before committing."));
    }

    #[test]
    fn test_system_prompt_explicit_skill_injection() {
        let dir = tempdir().unwrap();
        let skills_dir = dir.path().join(".m31a").join("skills").join("benchmarker");
        fs::create_dir_all(&skills_dir).unwrap();
        let skill_file = skills_dir.join("SKILL.md");
        fs::write(
            &skill_file,
            "---\nname: benchmarker\ndescription: Measures latency and throughput\n---\nRun criterion benchmarks with 100 samples.",
        )
        .unwrap();

        let prompt = SystemPromptBuilder::build_with_user_prompt(
            dir.path(),
            "meta/llama-3.3-70b-instruct",
            "WorkspaceWrite",
            CollaborationMode::Default,
            Some("Please execute $benchmarker on the crate"),
        );

        assert!(prompt.contains("Active Skill Instructions (Explicitly Requested)"));
        assert!(prompt.contains("Skill: benchmarker"));
        assert!(prompt.contains("Run criterion benchmarks with 100 samples."));
    }

    #[test]
    fn test_system_prompt_goal_steering() {
        let dir = tempdir().unwrap();
        let goal = crate::goal_steering::GoalObjective::new(
            "thread-goal-123",
            "Refactor network proxy to support IPv6 subnets",
            Some(250_000),
        );

        let prompt = SystemPromptBuilder::build_with_goal(
            dir.path(),
            "meta/llama-3.3-70b-instruct",
            "WorkspaceWrite",
            CollaborationMode::Default,
            None,
            Some(&goal),
        );

        assert!(prompt.contains("<goal_steering>"));
        assert!(prompt.contains("Refactor network proxy to support IPv6 subnets"));
        assert!(prompt.contains("250000 budget"));
    }

    /// §5.3: the builder assembles named sections; identical inputs produce
    /// identical digests, and changing only the model changes exactly one.
    #[test]
    fn test_sections_diffing() {
        let dir = tempdir().unwrap();

        let state_a = SystemPromptBuilder::sections(
            dir.path(),
            "meta/llama-3.3-70b-instruct",
            "WorkspaceWrite",
            CollaborationMode::Default,
            None,
            None,
        );
        let state_b = SystemPromptBuilder::sections(
            dir.path(),
            "meta/llama-3.3-70b-instruct",
            "WorkspaceWrite",
            CollaborationMode::Default,
            None,
            None,
        );

        // Identical rebuild → empty diff, byte-identical render.
        assert!(state_a.diff_since(&state_b.section_digests()).is_empty());
        assert_eq!(state_a.render(), state_b.render());
        assert_eq!(
            state_a.render(),
            SystemPromptBuilder::build(
                dir.path(),
                "meta/llama-3.3-70b-instruct",
                "WorkspaceWrite",
                CollaborationMode::Default
            ),
            "sectioned render must be byte-identical to the joined builder"
        );

        // Model change → only the identity section changes.
        let state_c = SystemPromptBuilder::sections(
            dir.path(),
            "deepseek-ai/deepseek-r1",
            "WorkspaceWrite",
            CollaborationMode::Default,
            None,
            None,
        );
        let deltas = state_c.diff_since(&state_a.section_digests());
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].id, "identity");
        assert_eq!(deltas[0].kind, crate::world_state::DeltaKind::Changed);

        // Section identities are stable and ordered; context sections
        // (instructions/skills/memories/goal) render only when non-empty.
        let ids: Vec<String> = state_a
            .section_ids()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(
            ids[..4],
            ["identity", "directives", "patch_spec", "collaboration"]
        );
        for id in &ids[4..] {
            assert!(
                [
                    "instructions",
                    "skills",
                    "explicit_skills",
                    "memories",
                    "goal",
                    "dynamic_context",
                    "workspace_status"
                ]
                .contains(&id.as_str()),
                "unexpected context section: {id}"
            );
        }
    }

    #[test]
    fn test_dynamic_context_injection() {
        let dir = tempdir().unwrap();
        let item = DynamicContextItem {
            path: "src/main.rs".to_string(),
            content: "fn main() { println!(\"Hello dynamic context\"); }".to_string(),
            tokens: 15,
        };

        let state = SystemPromptBuilder::sections_with_dynamic_context(
            dir.path(),
            "meta/llama-3.3-70b-instruct",
            "WorkspaceWrite",
            CollaborationMode::Default,
            None,
            None,
            &[item],
        );

        let rendered = state.render();
        assert!(rendered.contains("## Dynamic Working Context"));
        assert!(rendered.contains("src/main.rs"));
        assert!(rendered.contains("Hello dynamic context"));

        let digests = state.section_digests();
        assert!(digests.contains_key("dynamic_context"));
    }
}
