//! Effective policy compiler and evaluation engine implementing PolicyGate (POL-01, POL-02, POL-03, D-01, D-02).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::kernel::seams::policy::{
    PolicyDecision, PolicyDecisionContract, PolicyError, PolicyEvaluationRequest, PolicyGate,
};
use crate::policy::defaults::{built_in_safety_rules, developer_defaults};
use crate::policy::layers::{PolicyLayer, merge_preliminary_decision};
use crate::policy::matcher::{PolicyEvaluationContext, PolicyMatcher};
use crate::policy::rule::{PolicyDocument, PolicyRule};

/// Authoritative policy decision record containing matched rule and audit provenance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyDecisionRecord {
    pub decision: PolicyDecision,
    pub matched_rule_id: Option<String>,
    pub matched_layer: Option<PolicyLayer>,
    pub precedence_rank: Option<u32>,
    pub authority_source: String,
    pub explanation: String,
    pub policy_version_or_hash: String,
}

impl PolicyDecisionRecord {
    pub fn to_contract(&self) -> PolicyDecisionContract {
        self.into()
    }
}

impl From<PolicyDecisionRecord> for PolicyDecisionContract {
    fn from(r: PolicyDecisionRecord) -> Self {
        Self {
            decision: r.decision,
            matched_rule_id: r.matched_rule_id,
            matched_layer: r.matched_layer.map(|l| l.name().to_string()),
            precedence_rank: r.precedence_rank,
            authority_source: r.authority_source,
            explanation: r.explanation,
            policy_version_or_hash: r.policy_version_or_hash,
        }
    }
}

impl From<&PolicyDecisionRecord> for PolicyDecisionContract {
    fn from(r: &PolicyDecisionRecord) -> Self {
        Self {
            decision: r.decision,
            matched_rule_id: r.matched_rule_id.clone(),
            matched_layer: r.matched_layer.map(|l| l.name().to_string()),
            precedence_rank: r.precedence_rank,
            authority_source: r.authority_source.clone(),
            explanation: r.explanation.clone(),
            policy_version_or_hash: r.policy_version_or_hash.clone(),
        }
    }
}

/// Compiled multi-layer policy engine implementing the PolicyGate trait.
#[derive(Debug, Clone)]
pub struct EffectivePolicy {
    layers: Vec<(PolicyLayer, Vec<PolicyRule>)>,
    active_policy_hash: String,
    default_fallback: PolicyDecision,
}

use crate::policy::file::PolicyFileError;

/// Error encountered when compiling or loading the authoritative policy hierarchy.
#[derive(Debug, thiserror::Error)]
pub enum PolicyLoadError {
    #[error("failed to load {layer:?} policy from {path}: {source}")]
    LayerLoadError {
        layer: PolicyLayer,
        path: PathBuf,
        #[source]
        source: PolicyFileError,
    },
    #[error("policy file unreadable at {path}: {message}")]
    Unreadable {
        layer: PolicyLayer,
        path: PathBuf,
        message: String,
    },
    #[error(
        "legacy policy file found at {path} for layer {layer:?}; migration to canonical Markdown is required"
    )]
    LegacyMigrationRequired { layer: PolicyLayer, path: PathBuf },
}

fn load_first_candidate_policy(
    candidates: &[PathBuf],
    layer: PolicyLayer,
) -> Result<Option<Vec<PolicyRule>>, PolicyLoadError> {
    let mut found_md: Option<PathBuf> = None;
    let mut found_legacy: Option<PathBuf> = None;

    for candidate in candidates {
        if candidate.exists() {
            if candidate.extension().is_some_and(|e| e == "md") {
                if found_md.is_none() {
                    found_md = Some(candidate.clone());
                }
            } else if found_legacy.is_none() {
                found_legacy = Some(candidate.clone());
            }
        }
    }

    let target_path = if let Some(md) = found_md {
        if let Some(legacy) = found_legacy {
            tracing::warn!(
                "Both canonical Markdown ('{}') and legacy TOML ('{}') policy files exist for layer {:?}. Canonical format took precedence.",
                md.display(),
                legacy.display(),
                layer
            );
        }
        Some(md)
    } else if let Some(legacy) = found_legacy {
        return Err(PolicyLoadError::LegacyMigrationRequired {
            layer,
            path: legacy,
        });
    } else {
        None
    };

    if let Some(path) = target_path {
        let doc = PolicyDocument::load_from_file(&path).map_err(|source| {
            PolicyLoadError::LayerLoadError {
                layer,
                path: path.clone(),
                source,
            }
        })?;
        Ok(Some(doc.rules))
    } else {
        Ok(None)
    }
}

impl EffectivePolicy {
    /// Create a new EffectivePolicy from explicit layers.
    /// Layers are automatically sorted in ascending order of PolicyLayer (highest authority first).
    pub fn new(layers: Vec<(PolicyLayer, Vec<PolicyRule>)>) -> Self {
        Self::new_with_fallback(layers, PolicyDecision::Ask)
    }

    /// Create a new EffectivePolicy with explicit layers and a custom fallback decision.
    pub fn new_with_fallback(
        mut layers: Vec<(PolicyLayer, Vec<PolicyRule>)>,
        default_fallback: PolicyDecision,
    ) -> Self {
        layers.sort_by_key(|(layer, _)| *layer);

        let mut hasher = Sha256::new();
        hasher.update(format!("{:?}", default_fallback).as_bytes());
        for (layer, rules) in &layers {
            hasher.update((layer.precedence_rank() as u64).to_be_bytes());
            for rule in rules {
                hasher.update(rule.id.as_bytes());
                hasher.update(format!("{:?}", rule.decision).as_bytes());
                for tool in &rule.tools {
                    hasher.update(tool.as_bytes());
                }
                for path in &rule.paths {
                    hasher.update(path.as_bytes());
                }
                if let Some(args) = &rule.args {
                    hasher.update(args.to_string().as_bytes());
                }
            }
        }
        let active_policy_hash = format!("{:x}", hasher.finalize());

        Self {
            layers,
            active_policy_hash,
            default_fallback,
        }
    }

    /// Builder for compiling the canonical policy hierarchy.
    pub fn builder() -> EffectivePolicyBuilder {
        EffectivePolicyBuilder::default()
    }

    /// Restricted fail-closed policy stack:
    /// Contains only Layer 0 BuiltInSafety rules and a default fallback of PolicyDecision::Deny.
    /// No permissive developer defaults are applied.
    pub fn restricted() -> Self {
        let mut builder = Self::builder();
        builder = builder.with_layer(PolicyLayer::BuiltInSafety, built_in_safety_rules());
        builder = builder.with_default_fallback(PolicyDecision::Deny);
        builder.build()
    }

    /// Compile a standard policy stack using built-in safety, filesystem configs, and developer defaults.
    pub fn standard(workspace_root: &Path) -> Result<Self, PolicyLoadError> {
        Self::standard_with_policy_config(workspace_root, None)
    }

    /// Explicit try variant for compiling a standard policy stack.
    pub fn try_standard(workspace_root: &Path) -> Result<Self, PolicyLoadError> {
        Self::standard_with_policy_config(workspace_root, None)
    }

    /// Compile a standard policy stack with an optional PolicyConfig.
    pub fn standard_with_policy_config(
        workspace_root: &Path,
        policy_config: Option<&crate::config::PolicyConfig>,
    ) -> Result<Self, PolicyLoadError> {
        Self::try_standard_with_policy_config(workspace_root, policy_config)
    }

    /// Explicit try variant for compiling a standard policy stack with an optional PolicyConfig.
    pub fn try_standard_with_policy_config(
        workspace_root: &Path,
        policy_config: Option<&crate::config::PolicyConfig>,
    ) -> Result<Self, PolicyLoadError> {
        let mut builder = Self::builder();

        // Layer 0: Built-in safety invariants
        builder = builder.with_layer(PolicyLayer::BuiltInSafety, built_in_safety_rules());

        let platform_paths = crate::config::PlatformPaths::new();

        // Layer 1: System admin policy (delegates candidate resolution to PlatformPaths)
        if let Some(rules) = load_first_candidate_policy(
            &platform_paths.system_policy_candidates(),
            PolicyLayer::SystemAdmin,
        )? {
            builder = builder.with_layer(PolicyLayer::SystemAdmin, rules);
        }

        // Layer 2: Organization policy (delegates candidate resolution to PlatformPaths)
        if let Some(rules) = load_first_candidate_policy(
            &platform_paths.organization_policy_candidates(),
            PolicyLayer::Organization,
        )? {
            builder = builder.with_layer(PolicyLayer::Organization, rules);
        }

        // Layer 3: Workspace policy (delegates candidate resolution to PlatformPaths)
        if let Some(rules) = load_first_candidate_policy(
            &crate::config::PlatformPaths::workspace_policy_candidates(workspace_root),
            PolicyLayer::Workspace,
        )? {
            builder = builder.with_layer(PolicyLayer::Workspace, rules);
        }

        // Layer 4: User policy (delegates candidate resolution to PlatformPaths)
        if let Some(rules) = load_first_candidate_policy(
            &platform_paths.user_policy_candidates(),
            PolicyLayer::User,
        )? {
            builder = builder.with_layer(PolicyLayer::User, rules);
        }

        // Config-level denied tools (injected into Workspace policy layer)
        if let Some(p_cfg) = policy_config
            && !p_cfg.denied_tools.is_empty()
        {
            let mut rules = Vec::new();
            for denied in &p_cfg.denied_tools {
                let mut rule =
                    PolicyRule::new(format!("config_denied_{}", denied), PolicyDecision::Deny);
                rule.tools.push(denied.clone());
                rule.description = Some(format!(
                    "Tool '{}' explicitly denied in configuration",
                    denied
                ));
                rules.push(rule);
            }
            builder = builder.with_layer(PolicyLayer::Workspace, rules);
        }

        // canonical interactive-approval input: user preference + immutable
        // floor. when `interactive_approvals` is true the effective policy
        // carries an explicit Ask rule for mutating tools so the setting is
        // observable (never decoration). when false no weakening rule is
        // added: built-in safety still governs and Ask never becomes Allow.
        if let Some(p_cfg) = policy_config
            && p_cfg.interactive_approvals
        {
            let mut rule = PolicyRule::new(
                "interactive_approvals_floor".to_string(),
                PolicyDecision::Ask,
            );
            for tool in [
                "workspace_fs_write",
                "shell_exec",
                "terminal_write",
                "terminal_write_input",
                "terminal_start_session",
                "terminal_resize",
                "terminal_terminate_session",
                "git_commit",
                "git_push",
            ] {
                rule.tools.push(tool.to_string());
            }
            rule.description = Some(
                "interactive approvals required for mutating tools (policy.interactive_approvals)"
                    .to_string(),
            );
            builder = builder.with_layer(PolicyLayer::Workspace, vec![rule]);
        }

        // Layer 9: POL-04 Developer defaults
        builder = builder.with_layer(PolicyLayer::DeveloperDefault, developer_defaults());

        if let Some(p_cfg) = policy_config {
            let configured = match p_cfg.default_action.to_lowercase().as_str() {
                "allow" => PolicyDecision::Allow,
                "deny" => PolicyDecision::Deny,
                _ => PolicyDecision::Ask,
            };
            // non-bypassable floor: interactive approvals require Ask-or-Deny
            // fallback; a configured Allow can never survive when the floor
            // is active (monotonic merger already rejects the config, this
            // is defense in depth at the policy compiler).
            let fallback = if p_cfg.interactive_approvals && configured == PolicyDecision::Allow {
                PolicyDecision::Ask
            } else {
                configured
            };
            builder = builder.with_default_fallback(fallback);
        }

        Ok(builder.build())
    }

    /// Active SHA-256 fingerprint of all compiled rules across all layers.
    pub fn active_policy_hash(&self) -> &str {
        &self.active_policy_hash
    }

    /// Extend this compiled policy with per-execution dynamic layers.
    ///
    /// Precedence is preserved by construction (`EffectivePolicy::new` sorts by
    /// layer rank): mission > agent-role > task > session-approval. Callers
    /// pass the rules derived from the current mission policy context,
    /// agent-role profile bounds, task constraints, and verified durable
    /// session grants. Empty vectors add no layer.
    pub fn extended_for_execution(
        &self,
        mission_rules: Vec<PolicyRule>,
        agent_role_rules: Vec<PolicyRule>,
        task_rules: Vec<PolicyRule>,
        session_grant_rules: Vec<PolicyRule>,
    ) -> Self {
        let mut layers = self.layers.clone();
        if !mission_rules.is_empty() {
            layers.push((PolicyLayer::Mission, mission_rules));
        }
        if !agent_role_rules.is_empty() {
            layers.push((PolicyLayer::AgentRole, agent_role_rules));
        }
        if !task_rules.is_empty() {
            layers.push((PolicyLayer::Task, task_rules));
        }
        if !session_grant_rules.is_empty() {
            layers.push((PolicyLayer::SessionApproval, session_grant_rules));
        }
        Self::new_with_fallback(layers, self.default_fallback)
    }

    /// Reference to compiled layers.
    pub fn layers(&self) -> &[(PolicyLayer, Vec<PolicyRule>)] {
        &self.layers
    }

    /// Authoritative evaluation of a request across all compiled layers.
    pub fn evaluate_request(
        &self,
        ctx: &PolicyEvaluationContext,
    ) -> (PolicyDecision, PolicyDecisionRecord) {
        let mut current_decision: Option<(PolicyDecision, PolicyLayer, String, String)> = None;

        for (layer, rules) in &self.layers {
            for rule in rules {
                if PolicyMatcher::matches_rule(rule, ctx) {
                    let rule_explanation = rule.description.clone().unwrap_or_else(|| {
                        format!("Matched rule '{}' at layer {:?}", rule.id, layer)
                    });

                    match current_decision.take() {
                        None => {
                            current_decision =
                                Some((rule.decision, *layer, rule.id.clone(), rule_explanation));
                        }
                        Some((high_dec, high_layer, high_rule_id, high_exp)) => {
                            let (merged_dec, merged_layer, merged_rule) =
                                merge_preliminary_decision(
                                    (high_dec, high_layer, high_rule_id.clone()),
                                    (rule.decision, *layer, rule.id.clone()),
                                );

                            let merged_exp = if merged_rule == rule.id {
                                rule_explanation
                            } else {
                                high_exp
                            };

                            current_decision =
                                Some((merged_dec, merged_layer, merged_rule, merged_exp));
                        }
                    }

                    // If we reached an immutable DENY at or above current layer, stop evaluation
                    if let Some((PolicyDecision::Deny, PolicyLayer::BuiltInSafety, _, _)) =
                        &current_decision
                    {
                        break;
                    }
                }
            }

            if let Some((PolicyDecision::Deny, PolicyLayer::BuiltInSafety, _, _)) =
                &current_decision
            {
                break;
            }
        }

        match current_decision {
            Some((decision, layer, rule_id, explanation)) => {
                let record = PolicyDecisionRecord {
                    decision,
                    matched_rule_id: Some(rule_id),
                    matched_layer: Some(layer),
                    precedence_rank: Some(layer.precedence_rank()),
                    authority_source: layer.authority_source().to_string(),
                    explanation,
                    policy_version_or_hash: self.active_policy_hash.clone(),
                };
                (decision, record)
            }
            None => {
                // No rule in any layer matched: fallback per configuration
                let record = PolicyDecisionRecord {
                    decision: self.default_fallback,
                    matched_rule_id: None,
                    matched_layer: None,
                    precedence_rank: None,
                    authority_source: "default fallback".to_string(),
                    explanation: format!(
                        "No policy rule matched; defaulting to {:?} per configuration",
                        self.default_fallback
                    ),
                    policy_version_or_hash: self.active_policy_hash.clone(),
                };
                (self.default_fallback, record)
            }
        }
    }
}

#[async_trait]
impl PolicyGate for EffectivePolicy {
    async fn evaluate(&self, req: PolicyEvaluationRequest) -> Result<PolicyDecision, PolicyError> {
        let ctx = PolicyEvaluationContext::try_from_request(&req)?;
        let (decision, _record) = self.evaluate_request(&ctx);
        Ok(decision)
    }

    async fn evaluate_record(
        &self,
        req: PolicyEvaluationRequest,
    ) -> Result<(PolicyDecision, Option<PolicyDecisionContract>), PolicyError> {
        let ctx = PolicyEvaluationContext::try_from_request(&req)?;
        let (decision, record) = self.evaluate_request(&ctx);
        Ok((decision, Some(record.into())))
    }

    fn policy_hash(&self) -> Option<String> {
        Some(self.active_policy_hash().to_string())
    }
}

/// Builder for constructing an EffectivePolicy.
#[derive(Debug)]
pub struct EffectivePolicyBuilder {
    layers: Vec<(PolicyLayer, Vec<PolicyRule>)>,
    default_fallback: PolicyDecision,
}

impl Default for EffectivePolicyBuilder {
    fn default() -> Self {
        Self {
            layers: Vec::new(),
            default_fallback: PolicyDecision::Ask,
        }
    }
}

impl EffectivePolicyBuilder {
    pub fn with_layer(mut self, layer: PolicyLayer, rules: Vec<PolicyRule>) -> Self {
        self.layers.push((layer, rules));
        self
    }

    pub fn with_default_fallback(mut self, fallback: PolicyDecision) -> Self {
        self.default_fallback = fallback;
        self
    }

    pub fn build(self) -> EffectivePolicy {
        EffectivePolicy::new_with_fallback(self.layers, self.default_fallback)
    }
}
