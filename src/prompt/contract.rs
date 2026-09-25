//! Versioned PromptContract models, hashing, and TOML deserialization.

use crate::prompt::context::MissionStage;
use crate::prompt::error::PromptError;
use crate::prompt::parameter::PromptParameter;
use crate::prompt::reference::PromptReference;
use crate::prompt::strategy::PromptStrategy;
use crate::prompt::v2::{
    AuthorityLevel, CompatibilityMetadata, EvidenceRequirements, FailureBehavior, FailurePolicy,
    OutputContract, PromptKind, PromptResponseKind, ReasoningDepth, ReasoningMode, ReasoningPolicy,
    VerificationRequirements,
};
use crate::state_machine::agent::AgentRole;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

/// Maximum permissible byte size for rendered prompt output (5 MB).
pub const MAX_RENDERED_BYTES: usize = 5 * 1024 * 1024;

/// Immutable P0 runtime safety invariants that cannot be altered or overridden by prompt contracts.
pub const RUNTIME_SAFETY_INVARIANTS: &str = "\
M31A RUNTIME SAFETY INVARIANTS (P0 - IMMUTABLE):
1. The model proposes. The runtime decides.
2. Every side effect must execute strictly through authorized tools and pass policy verification.
3. Completion requires genuine evidence; never fake success or emit placeholder implementations.
4. Direct mutation of the persistent database or bypassing the scheduler is prohibited.
5. All long-running operations must honor cancellation tokens and timeouts.";

/// Flexible deserializer for AgentRole that supports snake_case, PascalCase, and genesis role aliases.
pub fn deserialize_flexible_role<'de, D>(deserializer: D) -> Result<AgentRole, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct RoleVisitor;

    impl<'de> serde::de::Visitor<'de> for RoleVisitor {
        type Value = AgentRole;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str(
                "an agent role string (e.g. 'researcher', 'DiscoveryAnalyst', 'architect')",
            )
        }

        fn visit_str<E>(self, value: &str) -> Result<AgentRole, E>
        where
            E: serde::de::Error,
        {
            parse_role_flexible(value).map_err(serde::de::Error::custom)
        }
    }

    deserializer.deserialize_str(RoleVisitor)
}

/// Flexible deserializer for Option<AgentRole> that supports snake_case, PascalCase, and genesis role aliases.
pub fn deserialize_optional_flexible_role<'de, D>(
    deserializer: D,
) -> Result<Option<AgentRole>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        Some(s) => parse_role_flexible(&s)
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

/// Parses an agent role string with support for case-insensitivity, PascalCase, and upstream genesis role aliases.
///
/// Delegates to the open [`AgentRole`] identity parser: historic aliases
/// resolve to canonical ids, and any other well-formed id parses as an open
/// identity. Existence is enforced downstream by the role registry at
/// validation boundaries — this parser never rejects a well-formed id for
/// being unregistered.
pub fn parse_role_flexible(s: &str) -> Result<AgentRole, String> {
    s.parse::<AgentRole>()
}

/// Versioned, immutable contract governing an agent prompt template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptContract {
    /// Logical identifier (e.g. "genesis.discovery" or "agent.implementer.v2").
    pub id: String,
    /// Strict version number.
    pub version: u32,
    /// Assigned agent role executing this prompt.
    #[serde(deserialize_with = "deserialize_flexible_role")]
    pub role: AgentRole,
    /// Contract description and intent.
    pub description: String,
    /// Declared input parameters.
    pub input_parameters: Vec<PromptParameter>,
    /// Jinja2-compatible template body.
    pub template_body: String,
    /// Expected format of output (e.g. "markdown", "json", "verdict").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_output_format: Option<String>,
    /// SHA-256 hash of canonical contract contents.
    pub content_hash: String,

    // V2 Architectural Contract Fields
    /// Canonical PromptOS V2 category taxonomy.
    #[serde(default)]
    pub kind: PromptKind,
    /// Operating mission stage if specified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<MissionStage>,
    /// Default prompt execution strategy if specified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<PromptStrategy>,
    /// Authority level of this contract.
    #[serde(default)]
    pub authority: AuthorityLevel,
    /// Typed output contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_contract: Option<OutputContract>,
    /// Reasoning policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_policy: Option<ReasoningPolicy>,
    /// Empirical evidence requirements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_requirements: Option<EvidenceRequirements>,
    /// Independent verification and falsification requirements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_requirements: Option<VerificationRequirements>,
    /// Failure handling and retry policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_policy: Option<FailurePolicy>,
    /// Backward-compatibility and deprecation metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<CompatibilityMetadata>,
}

impl PromptContract {
    /// Construct and validate a standard PromptContract, automatically computing its content hash.
    pub fn new(
        id: impl Into<String>,
        version: u32,
        role: AgentRole,
        description: impl Into<String>,
        input_parameters: Vec<PromptParameter>,
        template_body: impl Into<String>,
        expected_output_format: Option<String>,
    ) -> Result<Self, PromptError> {
        let id_str = id.into();
        let desc_str = description.into();
        let body_str = template_body.into();

        if id_str.trim().is_empty()
            || !id_str
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        {
            return Err(PromptError::PromptInvalid {
                id: id_str,
                version,
                reason: "prompt contract id must be non-empty and contain only alphanumeric, '_', or '.'"
                    .to_string(),
            });
        }

        if version == 0 {
            return Err(PromptError::PromptInvalid {
                id: id_str,
                version,
                reason: "prompt contract version must be >= 1".to_string(),
            });
        }

        if desc_str.trim().is_empty() {
            return Err(PromptError::PromptInvalid {
                id: id_str,
                version,
                reason: "prompt contract description cannot be empty".to_string(),
            });
        }

        if body_str.len() > 524_288 {
            return Err(PromptError::PromptInvalid {
                id: id_str,
                version,
                reason: format!(
                    "prompt template body size ({}) exceeds limit (512 KB)",
                    body_str.len()
                ),
            });
        }

        // Validate parameter uniqueness and completeness
        let mut seen = HashSet::new();
        for param in &input_parameters {
            if param.name.trim().is_empty() {
                return Err(PromptError::PromptInvalid {
                    id: id_str,
                    version,
                    reason: "parameter name cannot be empty".to_string(),
                });
            }
            if !seen.insert(param.name.clone()) {
                return Err(PromptError::PromptInvalid {
                    id: id_str,
                    version,
                    reason: format!("duplicate parameter name '{}'", param.name),
                });
            }
        }

        // Validate MiniJinja template syntax
        let mut env = minijinja::Environment::new();
        env.add_template(&id_str, &body_str)
            .map_err(|e| PromptError::PromptInvalid {
                id: id_str.clone(),
                version,
                reason: format!("MiniJinja template syntax error: {}", e),
            })?;

        let kind = if id_str.starts_with("core.") || id_str.starts_with("runtime.") {
            PromptKind::Core
        } else if id_str.starts_with("agent.") {
            PromptKind::Agent
        } else if id_str.starts_with("planning.") {
            PromptKind::Planning
        } else if id_str.starts_with("execution.") {
            PromptKind::Execution
        } else if id_str.starts_with("verification.") {
            PromptKind::Verification
        } else if id_str.starts_with("recovery.") {
            PromptKind::Recovery
        } else if id_str.starts_with("genesis.") {
            PromptKind::Genesis
        } else if id_str.starts_with("skill.") {
            PromptKind::Skill
        } else if matches!(
            id_str.as_str(),
            "implement" | "review" | "verify" | "diagnose"
        ) {
            PromptKind::Compatibility
        } else {
            PromptKind::Execution
        };

        let authority = if kind == PromptKind::Core {
            AuthorityLevel::Kernel
        } else if kind == PromptKind::Agent {
            AuthorityLevel::RoleContract
        } else if kind == PromptKind::Planning || kind == PromptKind::Genesis {
            AuthorityLevel::StageContract
        } else {
            AuthorityLevel::TaskContract
        };

        let content_hash = Self::calculate_hash(
            &id_str,
            version,
            role.clone(),
            &desc_str,
            &input_parameters,
            &body_str,
            expected_output_format.as_deref(),
        );

        Ok(Self {
            id: id_str,
            version,
            role,
            description: desc_str,
            input_parameters,
            template_body: body_str,
            expected_output_format,
            content_hash,
            kind,
            stage: None,
            strategy: None,
            authority,
            output_contract: None,
            reasoning_policy: None,
            evidence_requirements: None,
            verification_requirements: None,
            failure_policy: None,
            compatibility: None,
        })
    }

    /// Construct a full PromptOS V2 PromptContract with all typed architectural metadata.
    #[allow(clippy::too_many_arguments)]
    pub fn new_v2(
        id: impl Into<String>,
        version: u32,
        role: AgentRole,
        kind: PromptKind,
        stage: Option<MissionStage>,
        description: impl Into<String>,
        authority: AuthorityLevel,
        input_parameters: Vec<PromptParameter>,
        template_body: impl Into<String>,
        output_contract: Option<OutputContract>,
        reasoning_policy: Option<ReasoningPolicy>,
        evidence_requirements: Option<EvidenceRequirements>,
        verification_requirements: Option<VerificationRequirements>,
        failure_policy: Option<FailurePolicy>,
        compatibility: Option<CompatibilityMetadata>,
    ) -> Result<Self, PromptError> {
        let id_str = id.into();
        let desc_str = description.into();
        let body_str = template_body.into();

        if id_str.trim().is_empty()
            || !id_str
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        {
            return Err(PromptError::PromptInvalid {
                id: id_str,
                version,
                reason: "prompt contract id must be non-empty and contain only alphanumeric, '_', or '.'"
                    .to_string(),
            });
        }

        if version == 0 {
            return Err(PromptError::PromptInvalid {
                id: id_str,
                version,
                reason: "prompt contract version must be >= 1".to_string(),
            });
        }

        if desc_str.trim().is_empty() {
            return Err(PromptError::PromptInvalid {
                id: id_str,
                version,
                reason: "prompt contract description cannot be empty".to_string(),
            });
        }

        // Validate parameter uniqueness
        let mut seen = HashSet::new();
        for param in &input_parameters {
            if !seen.insert(param.name.clone()) {
                return Err(PromptError::PromptInvalid {
                    id: id_str,
                    version,
                    reason: format!("duplicate parameter name '{}'", param.name),
                });
            }
        }

        // Validate MiniJinja template syntax
        let mut env = minijinja::Environment::new();
        env.add_template(&id_str, &body_str)
            .map_err(|e| PromptError::PromptInvalid {
                id: id_str.clone(),
                version,
                reason: format!("MiniJinja template syntax error: {}", e),
            })?;

        // Semantic validation of V2 invariants
        if let Some(ref ev) = evidence_requirements {
            if ev.required && ev.min_evidence_count == 0 {
                return Err(PromptError::PromptInvalid {
                    id: id_str,
                    version,
                    reason: "evidence.required is true but min_evidence_count is 0".to_string(),
                });
            }
        }

        let expected_output_format = output_contract.as_ref().map(|o| o.format_type.clone());

        let content_hash = Self::calculate_hash_v2(
            &id_str,
            version,
            kind,
            role.clone(),
            &desc_str,
            authority,
            &input_parameters,
            &body_str,
            expected_output_format.as_deref(),
            output_contract.as_ref(),
            evidence_requirements.as_ref(),
            verification_requirements.as_ref(),
            failure_policy.as_ref(),
        );

        Ok(Self {
            id: id_str,
            version,
            role,
            description: desc_str,
            input_parameters,
            template_body: body_str,
            expected_output_format,
            content_hash,
            kind,
            stage,
            strategy: None,
            authority,
            output_contract,
            reasoning_policy,
            evidence_requirements,
            verification_requirements,
            failure_policy,
            compatibility,
        })
    }

    /// Whether this is a V2 or higher contract.
    pub fn is_v2(&self) -> bool {
        self.version >= 2 || self.id.ends_with(".v2")
    }

    /// Whether this contract is officially marked deprecated.
    pub fn is_deprecated(&self) -> bool {
        self.compatibility
            .as_ref()
            .map(|c| c.is_deprecated)
            .unwrap_or(false)
    }

    /// Return canonical replacement ID if deprecated, or self.id.
    pub fn canonical_id(&self) -> &str {
        if let Some(ref comp) = self.compatibility {
            if let Some(ref repl) = comp.canonical_replacement {
                return repl.as_str();
            }
        }
        &self.id
    }

    /// Whether this contract obligates active falsification.
    pub fn requires_falsification(&self) -> bool {
        self.verification_requirements
            .as_ref()
            .map(|v| v.falsification_required)
            .unwrap_or(false)
    }

    /// Whether this contract requires empirical evidence.
    pub fn requires_evidence(&self) -> bool {
        self.evidence_requirements
            .as_ref()
            .map(|e| e.required)
            .unwrap_or(false)
    }

    /// Calculate deterministic SHA-256 hash of canonical V1 contract fields.
    pub fn calculate_hash(
        id: &str,
        version: u32,
        role: AgentRole,
        description: &str,
        input_parameters: &[PromptParameter],
        template_body: &str,
        expected_output_format: Option<&str>,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(id.as_bytes());
        hasher.update(b":");
        hasher.update(version.to_be_bytes());
        hasher.update(b":");
        hasher.update(role.as_str().as_bytes());
        hasher.update(b":");
        hasher.update(description.as_bytes());
        hasher.update(b":");

        // Sort parameters by name for deterministic hashing
        let mut sorted_params: Vec<&PromptParameter> = input_parameters.iter().collect();
        sorted_params.sort_by(|a, b| a.name.cmp(&b.name));
        for p in sorted_params {
            hasher.update(p.name.as_bytes());
            hasher.update(b"=");
            hasher.update(if p.is_required { b"req:" } else { b"opt:" });
            if let Some(def) = &p.default_value {
                hasher.update(def.as_bytes());
            }
            hasher.update(b";");
        }

        // Normalize template body newlines to '\n' for cross-platform determinism
        let normalized_body = template_body.replace("\r\n", "\n");
        hasher.update(normalized_body.as_bytes());
        hasher.update(b":");

        if let Some(fmt) = expected_output_format {
            hasher.update(fmt.as_bytes());
        }

        format!("{:x}", hasher.finalize())
    }

    /// Calculate deterministic SHA-256 hash of canonical V2 contract fields including evidence and authority.
    #[allow(clippy::too_many_arguments)]
    pub fn calculate_hash_v2(
        id: &str,
        version: u32,
        kind: PromptKind,
        role: AgentRole,
        description: &str,
        authority: AuthorityLevel,
        input_parameters: &[PromptParameter],
        template_body: &str,
        expected_output_format: Option<&str>,
        output_contract: Option<&OutputContract>,
        evidence_requirements: Option<&EvidenceRequirements>,
        verification_requirements: Option<&VerificationRequirements>,
        failure_policy: Option<&FailurePolicy>,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"V2:");
        hasher.update(id.as_bytes());
        hasher.update(b":");
        hasher.update(version.to_be_bytes());
        hasher.update(b":kind=");
        hasher.update(kind.as_str().as_bytes());
        hasher.update(b":role=");
        hasher.update(role.as_str().as_bytes());
        hasher.update(b":auth=");
        hasher.update([authority.rank()]);
        hasher.update(b":desc=");
        hasher.update(description.as_bytes());
        hasher.update(b":");

        let mut sorted_params: Vec<&PromptParameter> = input_parameters.iter().collect();
        sorted_params.sort_by(|a, b| a.name.cmp(&b.name));
        for p in sorted_params {
            hasher.update(p.name.as_bytes());
            hasher.update(b"=");
            hasher.update(if p.is_required { b"req:" } else { b"opt:" });
            if let Some(def) = &p.default_value {
                hasher.update(def.as_bytes());
            }
            hasher.update(b";");
        }

        let normalized_body = template_body.replace("\r\n", "\n");
        hasher.update(normalized_body.as_bytes());
        hasher.update(b":");

        if let Some(fmt) = expected_output_format {
            hasher.update(fmt.as_bytes());
        }

        if let Some(out) = output_contract {
            hasher.update(b"|out:");
            hasher.update(out.response_kind.as_str().as_bytes());
            if let Some(ref uri) = out.schema_uri {
                hasher.update(uri.as_bytes());
            }
        }

        if let Some(ev) = evidence_requirements {
            hasher.update(b"|ev:");
            hasher.update(if ev.required { b"req" } else { b"opt" });
            hasher.update((ev.min_evidence_count as u64).to_be_bytes());
        }

        if let Some(ver) = verification_requirements {
            hasher.update(b"|ver:");
            hasher.update(if ver.falsification_required {
                b"falsify".as_slice()
            } else {
                b"standard".as_slice()
            });
        }

        if let Some(fp) = failure_policy {
            hasher.update(b"|fail:");
            hasher.update(fp.max_retries.to_be_bytes());
        }

        format!("{:x}", hasher.finalize())
    }

    /// Parse a PromptContract from a TOML file format. Supports both V1 and V2 canonical structures.
    pub fn from_toml_str(toml_str: &str) -> Result<Self, PromptError> {
        #[derive(Deserialize)]
        struct PromptFileRaw {
            // Canonical format top-level fields
            #[serde(default)]
            id: Option<String>,
            #[serde(default)]
            version: Option<u32>,
            #[serde(default, deserialize_with = "deserialize_optional_flexible_role")]
            role: Option<AgentRole>,
            #[serde(default)]
            description: Option<String>,
            #[serde(default)]
            kind: Option<String>,
            #[serde(default)]
            stage: Option<String>,
            #[serde(default)]
            strategy: Option<String>,
            #[serde(default)]
            authority: Option<String>,
            #[serde(default)]
            inputs: Option<InputsRaw>,
            #[serde(default)]
            output: Option<OutputRaw>,
            #[serde(default)]
            reasoning: Option<ReasoningRaw>,
            #[serde(default)]
            evidence: Option<EvidenceRaw>,
            #[serde(default)]
            verification: Option<VerificationRaw>,
            #[serde(default)]
            failure_behavior: Option<FailureBehaviorRaw>,
            #[serde(default)]
            security: Option<SecurityRaw>,
            #[serde(default)]
            compatibility: Option<CompatibilityRaw>,
            #[serde(default)]
            body: Option<BodyRaw>,

            // Legacy format fields
            #[serde(default)]
            prompt: Option<PromptMetadataRaw>,
            #[serde(default)]
            parameters: Vec<PromptParameter>,
            #[serde(default)]
            template: Option<TemplateRaw>,
        }

        #[derive(Deserialize, Default)]
        struct InputsRaw {
            #[serde(default)]
            required: Vec<InputParamRequiredRaw>,
            #[serde(default)]
            optional: Vec<InputParamOptionalRaw>,
        }

        #[derive(Deserialize)]
        struct InputParamRequiredRaw {
            name: String,
            #[serde(default)]
            description: String,
            #[serde(default, rename = "type")]
            _type_name: Option<String>,
        }

        #[derive(Deserialize)]
        struct InputParamOptionalRaw {
            name: String,
            #[serde(default)]
            description: String,
            #[serde(default)]
            default: Option<String>,
            #[serde(default, rename = "type")]
            _type_name: Option<String>,
        }

        #[derive(Deserialize, Default)]
        struct OutputRaw {
            #[serde(rename = "type")]
            type_name: Option<String>,
            #[serde(default)]
            schema: Option<String>,
            #[serde(default)]
            strict: Option<bool>,
            #[serde(default)]
            response_kind: Option<String>,
        }

        #[derive(Deserialize, Default)]
        struct ReasoningRaw {
            #[serde(default)]
            mode: Option<String>,
            #[serde(default)]
            depth: Option<String>,
            #[serde(default)]
            mandatory_falsification: Option<bool>,
            #[serde(default)]
            claim_categorization: Option<bool>,
            #[serde(default)]
            chain_of_thought_budget: Option<usize>,
        }

        #[derive(Deserialize, Default)]
        struct EvidenceRaw {
            #[serde(default)]
            required: Option<bool>,
            #[serde(default)]
            min_evidence_count: Option<usize>,
            #[serde(default)]
            trusted_classes: Option<Vec<String>>,
            #[serde(default)]
            forbidden_assumptions: Option<Vec<String>>,
            #[serde(default)]
            freshness_required: Option<bool>,
            #[serde(default)]
            provenance_required: Option<bool>,
        }

        #[derive(Deserialize, Default)]
        struct VerificationRaw {
            #[serde(default)]
            required: Option<bool>,
            #[serde(default)]
            evidence_expected: Option<Vec<String>>,
            #[serde(default)]
            falsification_required: Option<bool>,
            #[serde(default)]
            counterexample_search: Option<bool>,
            #[serde(default)]
            reachability_check: Option<bool>,
            #[serde(default)]
            independent_execution: Option<bool>,
            #[serde(default)]
            required_gates: Option<Vec<String>>,
            #[serde(default)]
            acceptance_criteria: Option<Vec<String>>,
        }

        #[derive(Deserialize, Default)]
        struct FailureBehaviorRaw {
            #[serde(default)]
            behavior: Option<String>,
            #[serde(default)]
            max_retries: Option<u32>,
            #[serde(default)]
            escalation_path: Option<String>,
        }

        #[derive(Deserialize, Default)]
        struct SecurityRaw {
            #[serde(default)]
            _treat_context_as_untrusted: Option<bool>,
            #[serde(default)]
            runtime_authority: Option<bool>,
            #[serde(default)]
            immutable_safety_layer: Option<bool>,
        }

        #[derive(Deserialize, Default)]
        struct CompatibilityRaw {
            #[serde(default)]
            legacy_aliases: Option<Vec<String>>,
            #[serde(default)]
            is_deprecated: Option<bool>,
            #[serde(default)]
            canonical_replacement: Option<String>,
            #[serde(default)]
            deprecation_note: Option<String>,
        }

        #[derive(Deserialize)]
        struct BodyRaw {
            template: String,
        }

        #[derive(Deserialize)]
        struct PromptMetadataRaw {
            id: String,
            version: u32,
            #[serde(deserialize_with = "deserialize_flexible_role")]
            role: AgentRole,
            description: String,
            #[serde(default)]
            expected_output_format: Option<String>,
        }

        #[derive(Deserialize)]
        struct TemplateRaw {
            body: String,
        }

        let raw: PromptFileRaw =
            toml::from_str(toml_str).map_err(|e| PromptError::PromptInvalid {
                id: "<file>".to_string(),
                version: 0,
                reason: format!("failed to parse prompt TOML: {}", e),
            })?;

        // Extract metadata: legacy [prompt] table vs canonical top-level fields
        let (id, version, role, description, mut expected_output_format) =
            if let Some(p) = raw.prompt {
                (
                    p.id,
                    p.version,
                    p.role,
                    p.description,
                    p.expected_output_format,
                )
            } else {
                let id = raw.id.ok_or_else(|| PromptError::PromptInvalid {
                    id: "<file>".to_string(),
                    version: 0,
                    reason: "missing required 'id' in prompt contract".to_string(),
                })?;
                let version = raw.version.ok_or_else(|| PromptError::PromptInvalid {
                    id: id.clone(),
                    version: 0,
                    reason: "missing required 'version' in prompt contract".to_string(),
                })?;
                let role = raw.role.ok_or_else(|| PromptError::PromptInvalid {
                    id: id.clone(),
                    version,
                    reason: "missing required 'role' in prompt contract".to_string(),
                })?;
                let description = raw.description.ok_or_else(|| PromptError::PromptInvalid {
                    id: id.clone(),
                    version,
                    reason: "missing required 'description' in prompt contract".to_string(),
                })?;
                (id, version, role, description, None)
            };

        if version == 0 {
            return Err(PromptError::PromptInvalid {
                id: id.clone(),
                version: 0,
                reason: "contract version must be greater than 0".to_string(),
            });
        }

        // Validate and parse kind
        let kind = if let Some(k) = &raw.kind {
            PromptKind::parse_kind(k)?
        } else if id.starts_with("core.") || id.starts_with("runtime.") {
            PromptKind::Core
        } else if id.starts_with("agent.") {
            PromptKind::Agent
        } else if id.starts_with("planning.") {
            PromptKind::Planning
        } else if id.starts_with("execution.") {
            PromptKind::Execution
        } else if id.starts_with("verification.") {
            PromptKind::Verification
        } else if id.starts_with("recovery.") {
            PromptKind::Recovery
        } else if id.starts_with("genesis.") {
            PromptKind::Genesis
        } else if id.starts_with("skill.") {
            PromptKind::Skill
        } else if matches!(id.as_str(), "implement" | "review" | "verify" | "diagnose") {
            PromptKind::Compatibility
        } else {
            PromptKind::Execution
        };

        // Parse optional MissionStage. Accepts both canonical stage names
        // (matching MissionStage::as_str) and legacy descriptive aliases so
        // that contract-declared stages are authoritative and callers never
        // need a parallel role->stage table.
        let stage = if let Some(s) = &raw.stage {
            let norm = s.to_lowercase();
            match norm.as_str() {
                "discovery" | "research" => Some(MissionStage::Research),
                "synthesis" | "synthesize" => Some(MissionStage::Synthesize),
                "planning" | "plan" | "genesis" => Some(MissionStage::Plan),
                "plan_verify" => Some(MissionStage::PlanVerify),
                "execution" | "execute" => Some(MissionStage::Execute),
                "verification" | "verify" | "impl_verify" => Some(MissionStage::ImplVerify),
                "review" => Some(MissionStage::Review),
                "diagnosis" | "diagnose" | "recovery" => Some(MissionStage::Diagnose),
                "ship" => Some(MissionStage::Ship),
                "discuss" => Some(MissionStage::Discuss),
                _ => None,
            }
        } else {
            None
        };

        // Parse optional PromptStrategy
        let strategy = if let Some(strat) = &raw.strategy {
            let s = strat.to_lowercase();
            match s.as_str() {
                "minimal" => Some(PromptStrategy::Minimal),
                "standard" => Some(PromptStrategy::Standard),
                "deep_reasoning" => Some(PromptStrategy::DeepReasoning),
                "constrained" => Some(PromptStrategy::Constrained),
                "low_context" => Some(PromptStrategy::LowContext),
                "recovery" => Some(PromptStrategy::Recovery),
                _ => {
                    return Err(PromptError::PromptInvalid {
                        id: id.clone(),
                        version,
                        reason: format!("invalid prompt strategy '{}'", strat),
                    });
                }
            }
        } else {
            None
        };

        // Parse authority level
        let authority = if let Some(ref auth) = raw.authority {
            match auth.to_lowercase().as_str() {
                "kernel" | "0" => AuthorityLevel::Kernel,
                "system" | "system_policy" | "1" => AuthorityLevel::SystemPolicy,
                "role" | "role_contract" | "2" => AuthorityLevel::RoleContract,
                "stage" | "stage_contract" | "3" => AuthorityLevel::StageContract,
                "task" | "task_contract" | "4" => AuthorityLevel::TaskContract,
                "evidence" | "evidence_context" | "5" => AuthorityLevel::EvidenceContext,
                "mission" | "dynamic_mission" | "6" => AuthorityLevel::DynamicMission,
                other => {
                    return Err(PromptError::PromptInvalid {
                        id: id.clone(),
                        version,
                        reason: format!("invalid authority level '{}'", other),
                    });
                }
            }
        } else if kind == PromptKind::Core {
            AuthorityLevel::Kernel
        } else if kind == PromptKind::Agent {
            AuthorityLevel::RoleContract
        } else if kind == PromptKind::Planning || kind == PromptKind::Genesis {
            AuthorityLevel::StageContract
        } else {
            AuthorityLevel::TaskContract
        };

        // Validate reasoning
        let reasoning_policy = if let Some(ref reasoning) = raw.reasoning {
            let mode = if let Some(ref mode) = reasoning.mode {
                let m = mode.to_lowercase();
                match m.as_str() {
                    "direct" => ReasoningMode::Direct,
                    "step_by_step" => ReasoningMode::StepByStep,
                    "adaptive" => ReasoningMode::Adaptive,
                    _ => {
                        return Err(PromptError::PromptInvalid {
                            id: id.clone(),
                            version,
                            reason: format!("invalid reasoning.mode '{}'", mode),
                        });
                    }
                }
            } else {
                ReasoningMode::StepByStep
            };

            let depth = if let Some(ref depth) = reasoning.depth {
                let d = depth.to_lowercase();
                match d.as_str() {
                    "minimal" => ReasoningDepth::Minimal,
                    "standard" => ReasoningDepth::Standard,
                    "deep" => ReasoningDepth::Deep,
                    "task-dependent" | "task_dependent" => ReasoningDepth::TaskDependent,
                    _ => {
                        return Err(PromptError::PromptInvalid {
                            id: id.clone(),
                            version,
                            reason: format!("invalid reasoning.depth '{}'", depth),
                        });
                    }
                }
            } else {
                ReasoningDepth::Standard
            };

            Some(ReasoningPolicy {
                mode,
                depth,
                mandatory_falsification: reasoning.mandatory_falsification.unwrap_or(false),
                claim_categorization: reasoning.claim_categorization.unwrap_or(false),
                chain_of_thought_budget: reasoning.chain_of_thought_budget,
            })
        } else {
            None
        };

        // Validate security invariants
        if let Some(sec) = &raw.security {
            if let Some(false) = sec.runtime_authority {
                return Err(PromptError::PromptInvalid {
                    id: id.clone(),
                    version,
                    reason: "security.runtime_authority must be true".to_string(),
                });
            }
            if let Some(false) = sec.immutable_safety_layer {
                return Err(PromptError::PromptInvalid {
                    id: id.clone(),
                    version,
                    reason: "security.immutable_safety_layer must be true".to_string(),
                });
            }
        }

        // Determine output contract
        let output_contract = if let Some(ref out) = raw.output {
            let fmt = out
                .type_name
                .clone()
                .unwrap_or_else(|| "markdown".to_string());
            expected_output_format = Some(fmt.clone());
            let rkind = out
                .response_kind
                .as_deref()
                .map(PromptResponseKind::from_type_str)
                .unwrap_or_else(|| PromptResponseKind::from_type_str(&fmt));
            Some(OutputContract::new(
                rkind,
                fmt,
                out.schema.clone(),
                out.strict.unwrap_or(false),
            ))
        } else {
            None
        };

        // Parse evidence requirements
        let evidence_requirements = if let Some(ref ev) = raw.evidence {
            let req = ev.required.unwrap_or(false);
            let min_count = ev.min_evidence_count.unwrap_or(if req { 1 } else { 0 });
            if req && min_count == 0 {
                return Err(PromptError::PromptInvalid {
                    id: id.clone(),
                    version,
                    reason: "evidence.required is true but min_evidence_count is 0".to_string(),
                });
            }
            Some(EvidenceRequirements {
                required: req,
                min_evidence_count: min_count,
                trusted_classes: ev.trusted_classes.clone().unwrap_or_default(),
                forbidden_assumptions: ev.forbidden_assumptions.clone().unwrap_or_default(),
                freshness_required: ev.freshness_required.unwrap_or(true),
                provenance_required: ev.provenance_required.unwrap_or(true),
            })
        } else if let Some(ref ver) = raw.verification {
            if let Some(ref ee) = ver.evidence_expected {
                if !ee.is_empty() {
                    Some(EvidenceRequirements {
                        required: ver.required.unwrap_or(true),
                        min_evidence_count: ee.len(),
                        trusted_classes: ee.clone(),
                        forbidden_assumptions: Vec::new(),
                        freshness_required: true,
                        provenance_required: true,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Parse verification requirements
        let verification_requirements =
            raw.verification
                .as_ref()
                .map(|ver| VerificationRequirements {
                    falsification_required: ver.falsification_required.unwrap_or(false),
                    counterexample_search: ver.counterexample_search.unwrap_or(false),
                    reachability_check: ver.reachability_check.unwrap_or(false),
                    independent_execution: ver.independent_execution.unwrap_or(false),
                    required_gates: ver.required_gates.clone().unwrap_or_default(),
                    acceptance_criteria: ver.acceptance_criteria.clone().unwrap_or_default(),
                });

        // Parse failure policy
        let failure_policy = if let Some(ref fail) = raw.failure_behavior {
            let beh = if let Some(ref b) = fail.behavior {
                match b.to_lowercase().as_str() {
                    "retryable" | "retry" => FailureBehavior::Retryable,
                    "non_retryable" | "fail_closed" => FailureBehavior::NonRetryable,
                    "escalate" => FailureBehavior::Escalate,
                    "request_context" => FailureBehavior::RequestContext,
                    "request_verification" => FailureBehavior::RequestVerification,
                    _ => FailureBehavior::Retryable,
                }
            } else {
                FailureBehavior::Retryable
            };
            Some(FailurePolicy {
                behavior: beh,
                max_retries: fail.max_retries.unwrap_or(2),
                escalation_path: fail
                    .escalation_path
                    .clone()
                    .unwrap_or_else(|| "runtime.supervisor".to_string()),
            })
        } else {
            None
        };

        // Parse compatibility metadata
        let compatibility = raw
            .compatibility
            .as_ref()
            .map(|comp| CompatibilityMetadata {
                legacy_aliases: comp.legacy_aliases.clone().unwrap_or_default(),
                is_deprecated: comp.is_deprecated.unwrap_or(false),
                canonical_replacement: comp.canonical_replacement.clone(),
                deprecation_note: comp.deprecation_note.clone(),
            });

        // Extract input parameters
        let input_parameters = if let Some(inputs) = raw.inputs {
            let mut params = Vec::new();
            let mut seen = HashSet::new();
            for req in inputs.required {
                if !seen.insert(req.name.clone()) {
                    return Err(PromptError::PromptInvalid {
                        id: id.clone(),
                        version,
                        reason: format!("duplicate parameter '{}' in inputs", req.name),
                    });
                }
                params.push(PromptParameter {
                    name: req.name,
                    description: req.description,
                    is_required: true,
                    default_value: None,
                });
            }
            for opt in inputs.optional {
                if !seen.insert(opt.name.clone()) {
                    return Err(PromptError::PromptInvalid {
                        id: id.clone(),
                        version,
                        reason: format!(
                            "parameter '{}' declared multiple times or in both required and optional",
                            opt.name
                        ),
                    });
                }
                params.push(PromptParameter {
                    name: opt.name,
                    description: opt.description,
                    is_required: false,
                    default_value: Some(opt.default.unwrap_or_default()),
                });
            }
            params
        } else {
            raw.parameters
        };

        // Extract template body
        let template_body = if let Some(b) = raw.body {
            b.template
        } else if let Some(t) = raw.template {
            t.body
        } else {
            return Err(PromptError::PromptInvalid {
                id: id.clone(),
                version,
                reason: "missing '[body]' or '[template]' section in prompt contract".to_string(),
            });
        };

        // Validate MiniJinja template syntax and variable declarations
        let mut env = minijinja::Environment::new();
        env.add_template(&id, &template_body)
            .map_err(|e| PromptError::PromptInvalid {
                id: id.clone(),
                version,
                reason: format!("MiniJinja template syntax error: {}", e),
            })?;

        if let Ok(tmpl) = env.get_template(&id) {
            let declared_names: HashSet<&str> =
                input_parameters.iter().map(|p| p.name.as_str()).collect();
            for var in tmpl.undeclared_variables(false) {
                if !declared_names.contains(var.as_str()) {
                    return Err(PromptError::PromptInvalid {
                        id: id.clone(),
                        version,
                        reason: format!("template references undeclared parameter '{}'", var),
                    });
                }
            }
        }

        // For v1 contracts, use standard hash to preserve backward compatibility.
        // For v2 contracts, use calculate_hash_v2 to incorporate rich contract semantics.
        let content_hash = if version >= 2 {
            Self::calculate_hash_v2(
                &id,
                version,
                kind,
                role.clone(),
                &description,
                authority,
                &input_parameters,
                &template_body,
                expected_output_format.as_deref(),
                output_contract.as_ref(),
                evidence_requirements.as_ref(),
                verification_requirements.as_ref(),
                failure_policy.as_ref(),
            )
        } else {
            Self::calculate_hash(
                &id,
                version,
                role.clone(),
                &description,
                &input_parameters,
                &template_body,
                expected_output_format.as_deref(),
            )
        };

        Ok(Self {
            id,
            version,
            role,
            description,
            input_parameters,
            template_body,
            expected_output_format,
            content_hash,
            kind,
            stage,
            strategy,
            authority,
            output_contract,
            reasoning_policy,
            evidence_requirements,
            verification_requirements,
            failure_policy,
            compatibility,
        })
    }

    /// Returns the typed reference for this contract.
    pub fn reference(&self) -> PromptReference {
        PromptReference::new(&self.id, self.version)
    }
}
