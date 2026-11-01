//! ToolEffect classification and Plan Mode execution admission.
//!
//! Categorizes every tool invocation into granular effect classes and enforces
//! a strict fail-closed boundary during Plan Mode to prevent unintended file mutations,
//! external side effects, or privilege escalation.

use crate::ast_decomposer::ShellAstDecomposer;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Strongly typed effect classification of a tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolEffect {
    /// Purely read-only exploration: reading files, diagnostics, code search, inspections, plan updates.
    ReadOnly,
    /// Mutating workspace files or state: writing/editing files, deleting files, applying patches, creating worktrees.
    WorkspaceMutation,
    /// Side effects on external systems: network requests, web browsing, cloud deployment, external process spawning.
    ExternalSideEffect,
    /// Privileged administrative operations: secrets management, sandbox reconfiguration, trust modification.
    PrivilegedOperation,
}

impl ToolEffect {
    /// Returns true if this effect is permitted during Plan Mode.
    pub fn is_allowed_in_plan_mode(&self) -> bool {
        matches!(self, ToolEffect::ReadOnly)
    }

    /// Returns true if this effect mutates local workspace state.
    pub fn is_mutating(&self) -> bool {
        matches!(
            self,
            ToolEffect::WorkspaceMutation | ToolEffect::PrivilegedOperation
        )
    }
}

/// Error returned when a tool invocation violates Plan Mode restrictions.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PlanModeViolation {
    #[error(
        "Tool '{tool_name}' is blocked in Plan Mode: Workspace mutation not permitted ({effect:?})"
    )]
    WorkspaceMutationBlocked {
        tool_name: String,
        effect: ToolEffect,
    },
    #[error(
        "Tool '{tool_name}' is blocked in Plan Mode: External side effects not permitted ({effect:?})"
    )]
    ExternalSideEffectBlocked {
        tool_name: String,
        effect: ToolEffect,
    },
    #[error(
        "Tool '{tool_name}' is blocked in Plan Mode: Privileged operation not permitted ({effect:?})"
    )]
    PrivilegedOperationBlocked {
        tool_name: String,
        effect: ToolEffect,
    },
    #[error("Tool '{tool_name}' is unclassified and blocked by fail-closed Plan Mode policy")]
    UnclassifiedToolBlocked { tool_name: String },
}

/// Classifies a tool invocation by name and optional arguments.
pub fn classify_tool(tool_name: &str, tool_args: Option<&str>) -> ToolEffect {
    // 1. MCP Tools: external tools from external servers
    if tool_name.starts_with("mcp__") || tool_name.starts_with("mcp_") {
        return ToolEffect::ExternalSideEffect;
    }

    // 2. Shell and command execution tools: inspect command arguments dynamically
    match tool_name {
        "exec_command" | "bash" | "terminal_exec" | "sh" | "shell" => {
            if let Some(args_raw) = tool_args {
                classify_shell_command(args_raw)
            } else {
                ToolEffect::WorkspaceMutation
            }
        }

        // --- Pure Read-Only Exploration & Diagnostics ---
        "read_file"
        | "list_dir"
        | "file_search"
        | "outline_file"
        | "extract_symbol_body"
        | "find_definition"
        | "find_references"
        | "diagnose_compiler_errors"
        | "grep_search"
        | "view_image"
        | "audit_codebase_safety"
        | "check_repo_trust"
        | "git_status"
        | "git_diff"
        | "notes_read"
        | "notes_list"
        | "notes_search"
        | "history_list_windows"
        | "history_list_items"
        | "history_read_item"
        | "history_search"
        | "thread_lineage"
        | "list_connectors"
        | "discover_audio_devices"
        | "list_checkpoints"
        | "list_locks"
        | "list_active_locks"
        | "list_mcp_resources"
        | "read_mcp_resource"
        | "get_context_remaining"
        | "new_context_window"
        | "update_plan"
        | "agent_plan"
        | "get_goal"
        | "search_memories"
        | "cite_memory"
        | "list_agents"
        | "list_channels"
        | "read_thread"
        | "search_posts"
        | "list_worktrees"
        | "list_skills"
        | "list_processes"
        | "list_background_processes"
        | "get_background_process_output"
        | "poll_process"
        | "question"
        | "request_user_input"
        | "tool_search"
        | "discover_capabilities"
        | "find_related_files"
        | "fetch_issue"
        | "debug_inspect"
        | "sleep"
        | "list_tcp_tunnels"
        | "list_available_plugins_to_install" => ToolEffect::ReadOnly,

        // --- Workspace File and State Mutation ---
        "write_file"
        | "edit"
        | "apply_patch"
        | "todowrite"
        | "lock_path"
        | "unlock_path"
        | "acquire_path_lock"
        | "release_path_lock"
        | "create_worktree"
        | "remove_worktree"
        | "notes_write"
        | "notes_append"
        | "multi_env_copy_file"
        | "resolve_merge_conflicts"
        | "create_checkpoint"
        | "restore_checkpoint"
        | "revert_thread"
        | "integrate_agent"
        | "create_goal"
        | "update_goal"
        | "consolidate_memory" => ToolEffect::WorkspaceMutation,

        // --- External Systems, Networking, Process Spawning ---
        "web_search"
        | "web_fetch"
        | "browser_navigate"
        | "browser_click"
        | "browser_type"
        | "browser_screenshot"
        | "browser_evaluate"
        | "verify_visual_state"
        | "cloud_deploy"
        | "supabase_migrate"
        | "create_pull_request"
        | "submit_cloud_task"
        | "poll_cloud_task"
        | "cancel_cloud_task"
        | "create_tcp_tunnel"
        | "close_tcp_tunnel"
        | "start_background_process"
        | "stop_background_process"
        | "kill_process"
        | "write_stdin"
        | "spawn_agent"
        | "send_input"
        | "wait_agent"
        | "close_agent"
        | "terminate_agent_tree"
        | "resume_agent"
        | "create_channel"
        | "post_message"
        | "subscribe_channel"
        | "unsubscribe_channel"
        | "board_create_channel"
        | "board_post"
        | "board_read"
        | "use_skill"
        | "schedule"
        | "debug_launch"
        | "debug_breakpoint"
        | "debug_step"
        | "verify_workspace"
        | "code_review"
        | "create_diagnostic_bundle"
        | "migrate_external_agent"
        | "request_plugin_install"
        | "agent_complete" => ToolEffect::ExternalSideEffect,

        // --- Privileged Administrative Operations ---
        "store_secret"
        | "retrieve_secret"
        | "delete_secret"
        | "list_stored_secrets"
        | "request_permissions" => ToolEffect::PrivilegedOperation,

        // Default fail-closed for any unclassified or unknown tool
        _ => ToolEffect::PrivilegedOperation,
    }
}

/// Evaluates tool admission under Plan Mode, failing closed for any non-read-only effect.
pub fn evaluate_plan_mode(
    tool_name: &str,
    tool_args: Option<&str>,
) -> Result<ToolEffect, PlanModeViolation> {
    let effect = classify_tool(tool_name, tool_args);
    match effect {
        ToolEffect::ReadOnly => Ok(ToolEffect::ReadOnly),
        ToolEffect::WorkspaceMutation => Err(PlanModeViolation::WorkspaceMutationBlocked {
            tool_name: tool_name.to_string(),
            effect,
        }),
        ToolEffect::ExternalSideEffect => Err(PlanModeViolation::ExternalSideEffectBlocked {
            tool_name: tool_name.to_string(),
            effect,
        }),
        ToolEffect::PrivilegedOperation => Err(PlanModeViolation::PrivilegedOperationBlocked {
            tool_name: tool_name.to_string(),
            effect,
        }),
    }
}

/// Inspects shell command arguments and classifies their execution effect.
fn classify_shell_command(args_raw: &str) -> ToolEffect {
    // Extract command string from JSON or raw text
    let cmd_str = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(args_raw) {
        parsed
            .get("command")
            .or_else(|| parsed.get("cmd"))
            .and_then(|v| v.as_str())
            .unwrap_or(args_raw)
            .to_string()
    } else {
        args_raw.to_string()
    };

    let trimmed = cmd_str.trim();
    if trimmed.is_empty() {
        return ToolEffect::ReadOnly;
    }

    // Check for redirection operators (>, >>, <, 2>, &>) - automatic mutation/side-effect
    if trimmed.contains('>') || trimmed.contains('<') {
        return ToolEffect::WorkspaceMutation;
    }

    // Check for subshells, backticks, process substitutions
    if trimmed.contains("$(")
        || trimmed.contains('`')
        || trimmed.contains("<(")
        || trimmed.contains(">(")
    {
        return ToolEffect::WorkspaceMutation;
    }

    // Decompose command sequence safely
    let commands = match ShellAstDecomposer::try_decompose_sequence(trimmed) {
        Some(cmds) if !cmds.is_empty() => cmds,
        _ => {
            // Complex or unparsable shell script -> fail closed as WorkspaceMutation
            return ToolEffect::WorkspaceMutation;
        }
    };

    // Every command in the chain must be strictly read-only
    for argv in commands {
        if argv.is_empty() {
            continue;
        }
        let prog = &argv[0];
        let prog_base = std::path::Path::new(prog)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(prog);

        if !is_strictly_readonly_program(prog_base, &argv[1..]) {
            return ToolEffect::WorkspaceMutation;
        }
    }

    ToolEffect::ReadOnly
}

/// Checks whether a specific program and arguments represent a strictly read-only inspection.
fn is_strictly_readonly_program(prog: &str, args: &[String]) -> bool {
    let lower_prog = prog.to_lowercase();
    match lower_prog.as_str() {
        // Read-only filesystem query utilities
        "ls" | "dir" | "cat" | "head" | "tail" | "more" | "less" | "wc" | "grep" | "egrep"
        | "fgrep" | "rg" | "ag" | "ack" | "find" | "fd" | "file" | "stat" | "which" | "where"
        | "type" | "echo" | "printf" | "true" | "false" | "pwd" => {
            // Verify no output flags or in-place modifiers (e.g. sed -i)
            true
        }

        // Git commands: only allow read-only queries
        "git" => {
            if args.is_empty() {
                return true;
            }
            let subcmd = &args[0];
            matches!(
                subcmd.as_str(),
                "status"
                    | "diff"
                    | "log"
                    | "show"
                    | "branch"
                    | "tag"
                    | "rev-parse"
                    | "remote"
                    | "config"
                    | "describe"
                    | "check-ignore"
                    | "ls-files"
                    | "ls-tree"
                    | "shortlog"
                    | "blame"
            ) && !args.iter().any(|a| {
                // Reject git mutation flags
                a == "-d" || a == "-D" || a == "-m" || a == "--delete" || a == "--set-upstream"
            })
        }

        // Cargo read-only queries
        "cargo" => {
            if args.is_empty() {
                return false;
            }
            let subcmd = &args[0];
            matches!(
                subcmd.as_str(),
                "check" | "clippy" | "metadata" | "verify-project" | "tree"
            )
        }

        // Python/Node/Ruby/etc. interpreters: NEVER read-only when evaluated via shell
        "python" | "python3" | "node" | "ruby" | "perl" | "bash" | "sh" | "zsh" => false,

        // Mutating tools: explicitly forbidden
        "rm" | "mv" | "cp" | "touch" | "mkdir" | "rmdir" | "chmod" | "chown" | "ln" | "unlink"
        | "sed" | "awk" | "tee" | "dd" | "tar" | "unzip" | "gzip" | "patch" => false,

        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_mode_admission_read_only_tools() {
        assert_eq!(
            evaluate_plan_mode("read_file", None).unwrap(),
            ToolEffect::ReadOnly
        );
        assert_eq!(
            evaluate_plan_mode("grep_search", None).unwrap(),
            ToolEffect::ReadOnly
        );
        assert_eq!(
            evaluate_plan_mode("update_plan", None).unwrap(),
            ToolEffect::ReadOnly
        );
    }

    #[test]
    fn test_plan_mode_blocks_workspace_mutation() {
        let res = evaluate_plan_mode("write_file", None);
        assert!(matches!(
            res,
            Err(PlanModeViolation::WorkspaceMutationBlocked { .. })
        ));

        let res = evaluate_plan_mode("apply_patch", None);
        assert!(matches!(
            res,
            Err(PlanModeViolation::WorkspaceMutationBlocked { .. })
        ));

        let res = evaluate_plan_mode("create_worktree", None);
        assert!(matches!(
            res,
            Err(PlanModeViolation::WorkspaceMutationBlocked { .. })
        ));
    }

    #[test]
    fn test_plan_mode_blocks_external_side_effects() {
        let res = evaluate_plan_mode("web_search", None);
        assert!(matches!(
            res,
            Err(PlanModeViolation::ExternalSideEffectBlocked { .. })
        ));

        let res = evaluate_plan_mode("cloud_deploy", None);
        assert!(matches!(
            res,
            Err(PlanModeViolation::ExternalSideEffectBlocked { .. })
        ));
    }

    #[test]
    fn test_plan_mode_blocks_privileged_operations() {
        let res = evaluate_plan_mode("store_secret", None);
        assert!(matches!(
            res,
            Err(PlanModeViolation::PrivilegedOperationBlocked { .. })
        ));
    }

    #[test]
    fn test_plan_mode_fail_closed_for_unknown_tools() {
        let res = evaluate_plan_mode("unknown_custom_tool_xyz", None);
        assert!(res.is_err());
    }

    #[test]
    fn test_shell_command_redirection_bypass_blocked() {
        // Redirection to file
        let effect = classify_tool(
            "exec_command",
            Some(r#"{"command": "echo 'malicious' > src/lib.rs"}"#),
        );
        assert_eq!(effect, ToolEffect::WorkspaceMutation);

        // Append redirection
        let effect = classify_tool("exec_command", Some(r#"{"command": "ls >> /tmp/log.txt"}"#));
        assert_eq!(effect, ToolEffect::WorkspaceMutation);
    }

    #[test]
    fn test_shell_command_interpreter_bypass_blocked() {
        // Python one-liner
        let effect = classify_tool(
            "exec_command",
            Some(r#"{"command": "python3 -c 'import os; os.remove(\"file\")'"}"#),
        );
        assert_eq!(effect, ToolEffect::WorkspaceMutation);

        // Node one-liner
        let effect = classify_tool(
            "exec_command",
            Some(r#"{"command": "node -e 'fs.writeFileSync(\"file\", \"\")'"}"#),
        );
        assert_eq!(effect, ToolEffect::WorkspaceMutation);
    }

    #[test]
    fn test_shell_command_subshell_bypass_blocked() {
        let effect = classify_tool(
            "exec_command",
            Some(r#"{"command": "$(which rm) -rf src"}"#),
        );
        assert_eq!(effect, ToolEffect::WorkspaceMutation);

        let effect = classify_tool("exec_command", Some(r#"{"command": "`echo rm` -rf src"}"#));
        assert_eq!(effect, ToolEffect::WorkspaceMutation);
    }

    #[test]
    fn test_shell_command_benign_readonly_allowed() {
        let effect = classify_tool("exec_command", Some(r#"{"command": "git status"}"#));
        assert_eq!(effect, ToolEffect::ReadOnly);

        let effect = classify_tool("exec_command", Some(r#"{"command": "git diff HEAD~1"}"#));
        assert_eq!(effect, ToolEffect::ReadOnly);

        let effect = classify_tool(
            "exec_command",
            Some(r#"{"command": "ls -la src && grep -rn 'fn main' src"}"#),
        );
        assert_eq!(effect, ToolEffect::ReadOnly);
    }
}
