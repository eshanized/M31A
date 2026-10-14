//! Stage 6: Resource Scope & Lease Validation Stage (TL-03, per D-09).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::agent::runner::ActionRequest;
use crate::pipeline::error::ToolError;
use crate::pipeline::stages::capability_check::CapabilityAuthorizedState;
use crate::tools::definition::{AnyTool, ToolExecutionContext};
use crate::tools::risk::EffectiveRisk;

/// Validated canonical resource binding travelling the whole tool pipeline.
///
/// Security invariant (F-09): authorization applies to the EFFECTIVE
/// target (canonical path after symlink/ancestor resolution), not merely
/// the user-supplied lexical path. The binding is computed once in
/// [`ResourceScopeStage`] and carried through policy evaluation, approval,
/// and execution. The execution stage re-resolves the lexical path and
/// rejects the mutation when the live identity no longer matches the
/// authorized effective target (symlink-swap race guard).
#[derive(Debug, Clone)]
pub struct ResourceBinding {
    /// User-supplied lexical path (relative to the workspace, as decoded).
    pub lexical: PathBuf,
    /// Authorized effective target: workspace-relative canonical path after
    /// symlink and ancestor resolution at authorization time.
    pub effective: PathBuf,
}

/// Typed state envelope proving resource scoping succeeded.
pub struct ResourceScopedState {
    pub action: ActionRequest,
    pub tool: Arc<dyn AnyTool>,
    pub decoded_args: serde_json::Value,
    pub effective_risk: EffectiveRisk,
    /// Canonical resource bindings for every path-like argument, in
    /// decoded-argument order. Downstream stages MUST use these bindings
    /// for authorization and execution; they MUST NOT re-derive the target
    /// from raw arguments alone.
    pub bound_resources: Vec<ResourceBinding>,
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

        let mut primary_path: Option<(PathBuf, PathBuf)> = None;
        let mut all_bindings: Vec<ResourceBinding> = Vec::new();

        // Helper to validate a single path string against workspace boundaries and protected paths
        let validate_path_str = |path_str: &str| -> Result<(PathBuf, PathBuf), ToolError> {
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

            let mut effective_target = normalized.clone();

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

                        effective_target = canon_target
                            .strip_prefix(&canon_ws)
                            .map(Path::to_path_buf)
                            .unwrap_or(canon_target);
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

                                if let Ok(rem) = normalized.strip_prefix(&ancestor) {
                                    let resolved_full = canon_ancestor.join(rem);
                                    effective_target = resolved_full
                                        .strip_prefix(&canon_ws)
                                        .map(Path::to_path_buf)
                                        .unwrap_or(resolved_full);
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
            Ok((target_path.to_path_buf(), effective_target))
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
                        primary_path = Some(validated.clone());
                    }
                    all_bindings.push(ResourceBinding {
                        lexical: validated.0,
                        effective: validated.1,
                    });
                } else if key == "paths"
                    && let Some(arr) = val.as_array()
                {
                    for item in arr {
                        if let Some(path_str) = item.as_str() {
                            let validated = validate_path_str(path_str)?;
                            if primary_path.is_none() {
                                primary_path = Some(validated.clone());
                            }
                            all_bindings.push(ResourceBinding {
                                lexical: validated.0,
                                effective: validated.1,
                            });
                        }
                    }
                }
            }
        }

        // Evaluate EffectiveRisk: base risk escalated by sensitive path if applicable (F-09)
        let effective_risk = if let Some((ref lexical, ref effective)) = primary_path {
            crate::tools::risk::EffectiveRisk::evaluate_file_target(
                tool.base_risk(),
                lexical,
                Some(effective),
            )
        } else {
            EffectiveRisk::new(tool.base_risk())
        };

        Ok(ResourceScopedState {
            action: state.action,
            tool: state.tool,
            decoded_args: state.decoded_args,
            effective_risk,
            bound_resources: all_bindings,
        })
    }

    /// Re-verify bound resources immediately before execution (F-09 race guard).
    ///
    /// Re-resolves each lexical path against the live filesystem and rejects
    /// the operation when the live effective identity no longer matches the
    /// authorized effective target (e.g. a symlink was swapped between
    /// authorization and execution). Returns `Ok(())` when every binding
    /// still resolves to its authorized target.
    pub fn verify_bindings_live(
        bound_resources: &[ResourceBinding],
        workspace_root: &Path,
    ) -> Result<(), ToolError> {
        let canon_ws = workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.to_path_buf());
        for binding in bound_resources {
            let full = if binding.lexical.is_absolute() {
                binding.lexical.clone()
            } else {
                workspace_root.join(&binding.lexical)
            };
            let normalized = normalize_path(&full);
            // Resolve live effective target with the same ancestor logic as
            // authorization time.
            let live_effective: PathBuf = if normalized.exists() {
                match normalized.canonicalize() {
                    Ok(canon) => canon
                        .strip_prefix(&canon_ws)
                        .map(Path::to_path_buf)
                        .unwrap_or(canon),
                    Err(e) => {
                        return Err(ToolError::permission_denied(
                            "RESOURCE_BINDING_UNVERIFIABLE",
                            format!(
                                "Cannot re-resolve authorized target '{}': {e}",
                                binding.lexical.display()
                            ),
                            Some(
                                "Retry the operation; if the failure persists, inspect the path"
                                    .to_string(),
                            ),
                        )
                        .with_provenance("ResourceScopeStage"));
                    }
                }
            } else {
                let mut ancestor = normalized.clone();
                while !ancestor.exists() {
                    match ancestor.parent() {
                        Some(parent) => ancestor = parent.to_path_buf(),
                        None => break,
                    }
                }
                if !ancestor.exists() {
                    return Err(ToolError::permission_denied(
                        "RESOURCE_BINDING_UNVERIFIABLE",
                        format!(
                            "No existing ancestor for authorized target '{}'",
                            binding.lexical.display()
                        ),
                        None,
                    )
                    .with_provenance("ResourceScopeStage"));
                }
                match ancestor.canonicalize() {
                    Ok(canon_ancestor) => match normalized.strip_prefix(&ancestor) {
                        Ok(rem) => {
                            let resolved = canon_ancestor.join(rem);
                            resolved
                                .strip_prefix(&canon_ws)
                                .map(Path::to_path_buf)
                                .unwrap_or(resolved)
                        }
                        Err(_) => {
                            return Err(ToolError::permission_denied(
                                "RESOURCE_BINDING_MISMATCH",
                                format!(
                                    "Authorized target '{}' no longer resolves under its authorized ancestor",
                                    binding.lexical.display()
                                ),
                                None,
                            )
                            .with_provenance("ResourceScopeStage"));
                        }
                    },
                    Err(e) => {
                        return Err(ToolError::permission_denied(
                            "RESOURCE_BINDING_UNVERIFIABLE",
                            format!(
                                "Cannot re-resolve ancestor for '{}': {e}",
                                binding.lexical.display()
                            ),
                            None,
                        )
                        .with_provenance("ResourceScopeStage"));
                    }
                }
            };
            // Compare canonical identities: normalize both sides through the
            // workspace canonical root so lexical-vs-absolute spellings agree.
            let authorized_full = canon_ws.join(&binding.effective);
            let live_full = if live_effective.is_absolute() {
                live_effective.clone()
            } else {
                canon_ws.join(&live_effective)
            };
            let authorized_norm = normalize_path(&authorized_full);
            let live_norm = normalize_path(&live_full);
            if authorized_norm != live_norm {
                return Err(ToolError::permission_denied(
                    "RESOURCE_BINDING_MISMATCH",
                    format!(
                        "Target '{}' changed after authorization (authorized '{}', now '{}'); refusing mutation",
                        binding.lexical.display(),
                        binding.effective.display(),
                        live_effective.display()
                    ),
                    Some("The filesystem changed between authorization and execution; retry the operation".to_string()),
                )
                .with_provenance("ResourceScopeStage"));
            }
            // Re-check containment and protected components on the LIVE
            // target: a swap to an outside or protected target is denied even
            // if the spelling coincidentally matches.
            if !live_norm.starts_with(&canon_ws) {
                return Err(ToolError::permission_denied(
                    "PATH_OUT_OF_WORKSPACE",
                    format!(
                        "Live target '{}' escapes workspace root",
                        binding.lexical.display()
                    ),
                    None,
                )
                .with_provenance("ResourceScopeStage"));
            }
            if live_effective
                .strip_prefix(&canon_ws)
                .map(crate::capability::providers::local_fs::contains_protected_component)
                .unwrap_or_else(|_| {
                    crate::capability::providers::local_fs::contains_protected_component(
                        &live_effective,
                    )
                })
            {
                return Err(ToolError::permission_denied(
                    "PROTECTED_PATH_DENIED",
                    format!(
                        "Live target '{}' resolves to protected state (.git or .m31a)",
                        binding.lexical.display()
                    ),
                    None,
                )
                .with_provenance("ResourceScopeStage"));
            }
        }
        Ok(())
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
