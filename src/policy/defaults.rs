//! Hardcoded Layer 1 built-in safety vetoes and POL-04 developer defaults (POL-04, POL-06, D-02, D-04).

use crate::kernel::seams::policy::PolicyDecision;
use crate::policy::rule::PolicyRule;

/// Layer 1: Absolute, immutable built-in safety rules (vetoes).
///
/// Invariant: These rules are evaluated with highest authority (`PolicyLayer::BuiltInSafety = 0`)
/// and can NEVER be weakened, overridden, or bypassed by user, workspace, or session rules (POL-06).
pub fn built_in_safety_rules() -> Vec<PolicyRule> {
    vec![
        PolicyRule::new("veto-credential-access", PolicyDecision::Deny)
            .with_description("Deny all access to credentials, private keys, secrets, and environment variable files")
            .with_paths([
                "**/.ssh/**",
                "**/.aws/**",
                "**/.gnupg/**",
                "**/.env*",
                "**/*id_rsa*",
                "**/*id_ed25519*",
                "**/*token*",
                "**/*secret*",
            ]),
        PolicyRule::new("veto-system-policy-tampering", PolicyDecision::Deny)
            .with_description("Deny modifications to system policies, privilege escalation configs, and system account files")
            .with_paths([
                "/etc/m31/**",
                "/etc/sudoers*",
                "/etc/passwd",
                "/etc/shadow",
            ]),
        PolicyRule::new("veto-shell-profile-tampering", PolicyDecision::Deny)
            .with_description("Deny modifications to user and system shell startup profiles")
            .with_paths([
                "**/.bashrc",
                "**/.zshrc",
                "**/.profile",
                "/etc/profile",
                "**/.bash_profile",
            ]),
        PolicyRule::new("veto-protected-runtime-paths", PolicyDecision::Deny)
            .with_description("Deny all direct or indirect access to protected runtime and repository control paths (.git, .m31a)")
            .with_paths([
                "**/.git/**",
                "**/.git",
                "**/.m31a/**",
                "**/.m31a",
            ]),
        PolicyRule::new("veto-process-git-redirection", PolicyDecision::Deny)
            .with_description("Deny commands or arguments attempting to redirect Git repository or worktree")
            .with_tools([
                "run_command",
                "execute_command",
                "start_job",
                "shell:*",
                "process:*",
                "git:*",
            ])
            .with_args(serde_json::json!({
                "command": [
                    "*--git-dir*",
                    "*--work-tree*",
                    "*--separate-git-dir*",
                    "*GIT_DIR=*",
                    "*GIT_WORK_TREE=*",
                    "*GIT_INDEX_FILE=*",
                    "*core.gitDir*",
                    "*core.worktree*",
                ]
            })),
        PolicyRule::new("veto-protected-path-process-mutation", PolicyDecision::Deny)
            .with_description("Deny process or shell commands targeting protected paths (.git, .m31a)")
            .with_tools([
                "run_command",
                "execute_command",
                "start_job",
                "shell:*",
                "process:*",
            ])
            .with_args(serde_json::json!({
                "command": [
                    "* .git*",
                    "*/.git*",
                    "*.git/*",
                    "* .m31a*",
                    "*/.m31a*",
                    "*.m31a/*",
                    ".git",
                    ".m31a",
                ]
            })),
    ]
}

/// POL-04 Developer-friendly baseline defaults.
///
/// Compiled at Layer 9 (`PolicyLayer::DeveloperDefault`), serving as sensible fallbacks
/// that empower agent autonomy for typical engineering work while requiring approval for
/// outbound network access and external git pushes, and denying destructive host commands.
pub fn developer_defaults() -> Vec<PolicyRule> {
    vec![
        PolicyRule::new("dev-default-task-orchestration", PolicyDecision::Allow)
            .with_description("Allow internal autonomous task execution and orchestration")
            .with_tools([
                "execute_task",
                "complete",
                "finish",
                "done",
                "orchestration:*",
            ]),
        PolicyRule::new("dev-default-workspace-files", PolicyDecision::Allow)
            .with_description("Allow reading and mutating files inside the workspace root")
            .with_tools([
                "read_file",
                "write_file",
                "edit_file",
                "apply_patch",
                "replace_file_content",
                "list_dir",
                "glob",
                "grep_search",
                "find_by_name",
                "fs:*",
            ])
            .with_paths(["./**"]),
        PolicyRule::new("dev-default-qa-and-verification", PolicyDecision::Allow)
            .with_description("Allow test execution, linting, and verification in the workspace")
            .with_tools([
                "run_tests",
                "run_formatter",
                "run_linter",
                "qa:*",
                "verification:*",
                "cargo:*",
                "test:*",
            ]),
        PolicyRule::new("dev-default-git-local", PolicyDecision::Allow)
            .with_description("Allow local git inspect and staging operations")
            .with_tools([
                "git_status",
                "git_diff",
                "git_log",
                "git_show",
                "git_branch",
                "git_checkout",
                "git_add",
            ]),
        PolicyRule::new("dev-default-git-commit", PolicyDecision::Allow)
            .with_description("Allow local git commit operations")
            .with_tools(["git_commit", "git"])
            .with_args(serde_json::json!({ "subcommand": "commit" })),
        PolicyRule::new("dev-default-git-push", PolicyDecision::Ask)
            .with_description("Ask for operator approval before pushing to remote git repositories")
            .with_tools(["git_push", "git"])
            .with_args(serde_json::json!({ "subcommand": "push" })),
        PolicyRule::new(
            "dev-default-runtime-git-orchestration",
            PolicyDecision::Allow,
        )
        .with_description(
            "Allow bounded runtime-orchestrated git finalization (mission worktree \
                 setup, staging, commit, merge, and cleanup). These `runtime.*` actions \
                 are distinct from model-facing git tools: they execute only inside \
                 the governed mission lifecycle with exactly-bound gates, real \
                 provenance, and evidence capture. Higher-authority layers may \
                 Deny them (immutable veto) or set Ask (then a durable session \
                 grant or the mission execution authorization is required).",
        )
        .with_tools([
            "runtime.git_worktree_create",
            "runtime.git_worktree_remove",
            "runtime.git_finalize_stage",
            "runtime.git_commit_finalize",
            "runtime.git_merge_finalize",
            "runtime.git_cleanup",
        ]),
        PolicyRule::new("dev-default-recovery-restore", PolicyDecision::Allow)
            .with_description(
                "Allow bounded recovery restores of tracked files to the clean \
                 HEAD baseline (reconciliation and rollback). Repair proposals \
                 that write new content remain Ask-gated: only an explicit \
                 Allow rule or durable grant authorizes them.",
            )
            .with_tools(["recovery_reconcile_restore", "recovery_rollback_restore"]),
        PolicyRule::new("dev-default-sandboxed-shell", PolicyDecision::Allow)
            .with_description("Allow command execution within the sandbox environment")
            .with_tools(["run_command", "execute_command"]),
        PolicyRule::new("dev-default-network", PolicyDecision::Ask)
            .with_description("Ask for operator approval before external network/web access")
            .with_tools(["fetch_url", "web_search", "network:*"]),
        PolicyRule::new("dev-default-destructive-ops", PolicyDecision::Deny)
            .with_description("Deny catastrophic destructive shell commands")
            .with_tools(["run_command", "execute_command"])
            .with_args(serde_json::json!({
                "command": [
                    "*rm -rf /*",
                    "*git reset --hard*",
                    "*git clean -fdx*",
                ]
            })),
    ]
}
