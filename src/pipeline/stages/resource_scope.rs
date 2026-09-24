//! Stage 6: Resource Scope & Lease Validation Stage (TL-03, per D-09).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::agent::runner::ActionRequest;
use crate::pipeline::error::ToolError;
use crate::pipeline::stages::capability_check::CapabilityAuthorizedState;
use crate::tools::definition::{AnyTool, ToolExecutionContext};
use crate::tools::risk::EffectiveRisk;

/// Typed state envelope proving resource scoping succeeded.
pub struct ResourceScopedState {
    pub action: ActionRequest,
    pub tool: Arc<dyn AnyTool>,
    pub decoded_args: serde_json::Value,
    pub effective_risk: EffectiveRisk,
}

/// Stage 6: Verifies workspace path containment and evaluates dynamic `EffectiveRisk`.
pub struct ResourceScopeStage;

impl ResourceScopeStage {
    pub fn execute(
        state: CapabilityAuthorizedState,
        context: &ToolExecutionContext,
    ) -> Result<ResourceScopedState, ToolError> {
        let tool = &state.tool;
        let args = &state.decoded_args;
        let workspace_root = &context.workspace_root;

        let mut primary_path: Option<PathBuf> = None;

        // Helper to validate a single path string against workspace boundaries and protected paths
        let validate_path_str = |path_str: &str| -> Result<PathBuf, ToolError> {
            let target_path = Path::new(path_str);

            if crate::capability::providers::local_fs::contains_protected_component(target_path) {
                return Err(ToolError::permission_denied(
                    "PROTECTED_PATH_DENIED",
                    format!(
                        "Target path '{}' accesses protected repository or runtime state (.git or .m31a)",
                        path_str
                    ),
                    Some("Direct access to .git and .m31a is strictly prohibited".to_string()),
                )
                .with_provenance("ResourceScopeStage"));
            }

            let full_path = if target_path.is_absolute() {
                target_path.to_path_buf()
            } else {
                workspace_root.join(target_path)
            };

            let normalized = normalize_path(&full_path);
            let normalized_ws = normalize_path(workspace_root);

            if !normalized.starts_with(&normalized_ws) {
                return Err(ToolError::permission_denied(
                    "PATH_OUT_OF_WORKSPACE",
                    format!(
                        "Target path '{}' escapes workspace root '{}'",
                        path_str,
                        workspace_root.display()
                    ),
                    Some("Ensure all file paths stay within the workspace root".to_string()),
                )
                .with_provenance("ResourceScopeStage"));
            }

            if normalized
                .strip_prefix(&normalized_ws)
                .map(crate::capability::providers::local_fs::contains_protected_component)
                .unwrap_or(false)
            {
                return Err(ToolError::permission_denied(
                    "PROTECTED_PATH_DENIED",
                    format!(
                        "Target path '{}' resolves to protected repository or runtime state (.git or .m31a)",
                        path_str
                    ),
                    Some("Direct access to .git and .m31a is strictly prohibited".to_string()),
                )
                .with_provenance("ResourceScopeStage"));
            }

            if let Ok(canon_ws) = workspace_root.canonicalize() {
                if normalized.exists() {
                    if let Ok(canon_target) = normalized.canonicalize() {
                        if !canon_target.starts_with(&canon_ws) {
                            return Err(ToolError::permission_denied(
                                "PATH_OUT_OF_WORKSPACE",
                                format!(
                                    "Target path '{}' resolves outside workspace root '{}'",
                                    path_str,
                                    workspace_root.display()
                                ),
                                Some(
                                    "Ensure all file paths stay within the workspace root"
                                        .to_string(),
                                ),
                            )
                            .with_provenance("ResourceScopeStage"));
                        }

                        if canon_target.strip_prefix(&canon_ws).map(crate::capability::providers::local_fs::contains_protected_component).unwrap_or(false) {
                            return Err(ToolError::permission_denied(
                                "PROTECTED_PATH_DENIED",
                                format!(
                                    "Target path '{}' symlink resolves to protected repository or runtime state (.git or .m31a)",
                                    path_str
                                ),
                                Some("Direct access to .git and .m31a is strictly prohibited".to_string()),
                            )
                            .with_provenance("ResourceScopeStage"));
                        }
                    }
                } else {
                    // Target does not exist yet (e.g. creating new file through symlinked ancestor).
                    let mut ancestor = normalized.clone();
                    while !ancestor.exists() {
                        if let Some(parent) = ancestor.parent() {
                            ancestor = parent.to_path_buf();
                        } else {
                            break;
                        }
                    }
                    if ancestor.exists() {
                        match ancestor.canonicalize() {
                            Ok(canon_ancestor) => {
                                if !canon_ancestor.starts_with(&canon_ws) {
                                    return Err(ToolError::permission_denied(
                                        "PATH_OUT_OF_WORKSPACE",
                                        format!(
                                            "Target path '{}' resolves through ancestor '{}' outside workspace root '{}'",
                                            path_str,
                                            ancestor.display(),
                                            workspace_root.display()
                                        ),
                                        Some(
                                            "Ensure all file paths stay within the workspace root".to_string(),
                                        ),
                                    )
                                    .with_provenance("ResourceScopeStage"));
                                }

                                if canon_ancestor.strip_prefix(&canon_ws).map(crate::capability::providers::local_fs::contains_protected_component).unwrap_or(false) {
                                    return Err(ToolError::permission_denied(
                                        "PROTECTED_PATH_DENIED",
                                        format!(
                                            "Target path '{}' ancestor resolves to protected repository or runtime state (.git or .m31a)",
                                            path_str
                                        ),
                                        Some("Direct access to .git and .m31a is strictly prohibited".to_string()),
                                    )
                                    .with_provenance("ResourceScopeStage"));
                                }
                            }
                            Err(e) => {
                                return Err(ToolError::permission_denied(
                                    "CANONICALIZATION_FAILED",
                                    format!(
                                        "Failed to canonicalize ancestor directory '{}': {}",
                                        ancestor.display(),
                                        e
                                    ),
                                    None,
                                )
                                .with_provenance("ResourceScopeStage"));
                            }
                        }
                    }
                }
            }
            Ok(target_path.to_path_buf())
        };

        // Inspect parameters for command / args safety (P0 Process Boundary)
        if let Some(args_obj) = args.as_object() {
            if let Some(cmd_val) = args_obj.get("command").and_then(|c| c.as_str()) {
                let cmd_args: Vec<String> = args_obj
                    .get("args")
                    .and_then(|a| a.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|item| item.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                if let Err(violation) =
                    crate::process::env::check_command_safety(cmd_val, &cmd_args)
                {
                    let err_code = match violation {
                        crate::process::env::ProcessSecurityViolation::GitRedirection(_) => {
                            "GIT_REDIRECTION_DENIED"
                        }
                        _ => "PROTECTED_PATH_DENIED",
                    };
                    return Err(ToolError::permission_denied(
                        err_code,
                        format!("Process execution rejected: {violation}"),
                        Some(
                            "Execution targeting protected paths (.git, .m31a) or Git redirection is strictly prohibited"
                                .to_string(),
                        ),
                    )
                    .with_provenance("ResourceScopeStage"));
                }
            }

            for (key, val) in args_obj {
                if matches!(
                    key.as_str(),
                    "path" | "file_path" | "destination" | "target" | "cwd" | "file" | "dir"
                ) && let Some(path_str) = val.as_str()
                {
                    let validated = validate_path_str(path_str)?;
                    if primary_path.is_none() {
                        primary_path = Some(validated);
                    }
                } else if key == "paths"
                    && let Some(arr) = val.as_array()
                {
                    for item in arr {
                        if let Some(path_str) = item.as_str() {
                            let validated = validate_path_str(path_str)?;
                            if primary_path.is_none() {
                                primary_path = Some(validated);
                            }
                        }
                    }
                }
            }
        }

        // Evaluate EffectiveRisk: base risk escalated by sensitive path if applicable
        let effective_risk = if let Some(ref path) = primary_path {
            EffectiveRisk::evaluate_file_path(tool.base_risk(), path)
        } else {
            EffectiveRisk::new(tool.base_risk())
        };

        Ok(ResourceScopedState {
            action: state.action,
            tool: state.tool,
            decoded_args: state.decoded_args,
            effective_risk,
        })
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                components.pop();
            }
            c => components.push(c),
        }
    }
    components.into_iter().collect()
}
