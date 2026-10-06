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

    /// Compile a standard policy stack using built-in safety, filesystem configs, and developer defaults.
    pub fn standard(workspace_root: &Path) -> Self {
        Self::standard_with_policy_config(workspace_root, None)
    }

    /// Compile a standard policy stack with an optional PolicyConfig.
    pub fn standard_with_policy_config(
        workspace_root: &Path,
        policy_config: Option<&crate::config::PolicyConfig>,
    ) -> Self {
        let mut builder = Self::builder();

        // Layer 0: Built-in safety invariants
        builder = builder.with_layer(PolicyLayer::BuiltInSafety, built_in_safety_rules());

        let platform_paths = crate::config::PlatformPaths::new();
        let sys_config_dir = platform_paths.system_config_dir();

        // Layer 1: System admin policy (<system_config_dir>/policy.toml, fallback /etc/m31/policy.toml)
        let system_paths = [
            sys_config_dir.join("policy.toml"),
            PathBuf::from("/etc/m31/policy.toml"),
        ];
        for sys_path in &system_paths {
            if sys_path.exists()
                && let Ok(doc) = PolicyDocument::load_from_file(sys_path)
            {
                builder = builder.with_layer(PolicyLayer::SystemAdmin, doc.rules);
                break;
            }
        }

        // Layer 2: Organization policy (<system_config_dir>/organization.toml, fallback /etc/m31/organization.toml)
        let org_paths = [
            sys_config_dir.join("organization.toml"),
            PathBuf::from("/etc/m31/organization.toml"),
        ];
        for org_path in &org_paths {
            if org_path.exists()
                && let Ok(doc) = PolicyDocument::load_from_file(org_path)
            {
                builder = builder.with_layer(PolicyLayer::Organization, doc.rules);
                break;
            }
        }

        // Layer 3: Workspace policy (<workspace>/.m31a/policy.toml, fallback <workspace>/.m31/policy.toml)
        let workspace_paths = [
            workspace_root.join(".m31a/policy.toml"),
            workspace_root.join(".m31/policy.toml"),
        ];
        for ws_path in &workspace_paths {
            if ws_path.exists()
                && let Ok(doc) = PolicyDocument::load_from_file(ws_path)
            {
                builder = builder.with_layer(PolicyLayer::Workspace, doc.rules);
                break;
            }
        }

        // Layer 4: User policy (PlatformPaths, ~/.config/m31a/policy.toml, fallback ~/.config/m31/policy.toml)
        let mut user_candidates = Vec::new();
        user_candidates.push(
            crate::config::PlatformPaths::new()
                .config_dir()
                .join("policy.toml"),
        );
        if let Some(home) = std::env::var_os("HOME") {
            let home_path = std::path::PathBuf::from(home);
            user_candidates.push(home_path.join(".config/m31a/policy.toml"));
            user_candidates.push(home_path.join(".config/m31/policy.toml"));
        }
        for u_path in &user_candidates {
            if u_path.exists()
                && let Ok(doc) = PolicyDocument::load_from_file(u_path)
            {
                builder = builder.with_layer(PolicyLayer::User, doc.rules);
                break;
            }
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

        // Layer 9: POL-04 Developer defaults
        builder = builder.with_layer(PolicyLayer::DeveloperDefault, developer_defaults());

        if let Some(p_cfg) = policy_config {
            let fallback = match p_cfg.default_action.to_lowercase().as_str() {
                "allow" => PolicyDecision::Allow,
                "deny" => PolicyDecision::Deny,
                _ => PolicyDecision::Ask,
            };
            builder = builder.with_default_fallback(fallback);
        }

        builder.build()
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
