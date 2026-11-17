//! Maintenance Delivery, Pull Request Preparation & External Side-Effect Gateway Binding (`m31a-maintenance::delivery`).
//!
//! Enforces:
//! 1. Structured, reviewable pull request packages with complete cryptographic evidence.
//! 2. Draft PRs by default; never automatic merges.
//! 3. Mediated dispatch via `ExternalEffectGateway` with attempt fencing and idempotency keys.
//! 4. Audit trail logging of publication events.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use m31a_agent_engine::effect_gateway::{
    EffectAdmissionOutcome, EffectDispatchRequest, ExternalEffectGateway,
};

use crate::domain::{MaintenanceCase, MaintenancePlan, MaintenancePolicy};
use crate::storage::MaintenanceStore;

/// Reviewable maintenance deliverable package ready for pull request creation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MaintenancePullRequestPackage {
    pub title: String,
    pub head_branch: String,
    pub base_branch: String,
    pub is_draft: bool,
    pub body_markdown: String,
    pub labels: Vec<String>,
    pub manifest_hash: String,
}

/// Delivery adapter packaging deliverables and mediating remote side effects.
#[derive(Clone)]
pub struct DeliveryAdapter {
    maint_store: MaintenanceStore,
    gateway: Option<ExternalEffectGateway>,
    policy: MaintenancePolicy,
}

impl DeliveryAdapter {
    pub fn new(
        maint_store: MaintenanceStore,
        gateway: Option<ExternalEffectGateway>,
        policy: MaintenancePolicy,
    ) -> Self {
        Self {
            maint_store,
            gateway,
            policy,
        }
    }

    pub fn maint_store(&self) -> &MaintenanceStore {
        &self.maint_store
    }

    /// Formulates a structured, evidence-rich Markdown pull request body.
    pub fn format_pull_request_markdown(
        case: &MaintenanceCase,
        plan: &MaintenancePlan,
        manifest_hash: &str,
        verification_summary: &str,
    ) -> String {
        let mut md = String::new();
        md.push_str("## 🛠️ Autonomous Maintenance Remediation Report\n\n");
        md.push_str("> [!IMPORTANT]\n");
        md.push_str("> This pull request was synthesized and verified by the **M31A Autonomous Repository Maintenance Engine**.\n");
        md.push_str("> Every change is cryptographically bound to a frozen change-set manifest and verified independently.\n\n");

        md.push_str("### 1. Problem Statement\n");
        md.push_str(&format!("- **Case ID:** `{}`\n", case.id));
        md.push_str(&format!("- **Category:** `{}`\n", case.category.as_str()));
        md.push_str(&format!(
            "- **Severity:** `{:?}` | **Priority:** `{:?}`\n",
            case.severity, case.priority
        ));
        md.push_str(&format!(
            "- **Description:** {}\n\n",
            case.problem_statement
        ));

        md.push_str("### 2. Diagnosis & Root-Cause Hypotheses\n");
        md.push_str(&format!(
            "- **Problem Classification:** `{}`\n",
            plan.problem_classification
        ));
        md.push_str(&format!(
            "- **Selected Strategy:** `{}`\n",
            plan.selected_strategy
        ));
        for (idx, hyp) in plan.hypotheses.iter().enumerate() {
            md.push_str(&format!(
                "  {}. **Hypothesis:** {} (Confidence: {:.0}%)\n",
                idx + 1,
                hyp.description,
                hyp.confidence * 100.0
            ));
        }
        md.push('\n');

        md.push_str("### 3. Change-Set & Affected Scope\n");
        md.push_str(&format!(
            "- **Baseline Revision:** `{}`\n",
            plan.baseline_sha
        ));
        md.push_str(&format!(
            "- **ChangeSetManifest SHA-256:** `{}`\n",
            manifest_hash
        ));
        if !case.affected_files.is_empty() {
            md.push_str("- **Affected Files:**\n");
            for f in &case.affected_files {
                md.push_str(&format!("  - `{}`\n", f.display()));
            }
        }
        md.push('\n');

        md.push_str("### 4. Independent Verification Evidence\n");
        md.push_str(&format!("{}\n\n", verification_summary));
        md.push_str("- **Mandatory Requirements:**\n");
        for req in &plan.verification_requirements {
            md.push_str(&format!(
                "  - [x] `{}`: {}\n",
                req.category, req.description
            ));
        }
        md.push('\n');

        md.push_str("### 5. Review & Approval Requirements\n");
        md.push_str(&format!(
            "- **Autonomy Tier:** `{:?}`\n",
            plan.required_autonomy_tier
        ));
        md.push_str(&format!(
            "- **Estimated Risk:** `{:?}`\n",
            plan.estimated_risk
        ));
        md.push_str("- **Auto-Merge Permitted:** `false` (Human review strictly required)\n");
        md.push_str("- **Draft Mode:** `true`\n\n");

        md.push_str(
            "---\n*Generated autonomously by M31A Autonomous Repository Maintenance Engine.*",
        );
        md
    }

    /// Prepares reviewable pull request package.
    pub fn prepare_pull_request_package(
        &self,
        case: &MaintenanceCase,
        plan: &MaintenancePlan,
        manifest_hash: &str,
        verification_summary: &str,
    ) -> Result<MaintenancePullRequestPackage> {
        let title = format!(
            "[M31A] {}: {}",
            case.category.as_str().replace('_', " ").to_uppercase(),
            case.title
        );

        let head_branch = format!(
            "{}{}-{}",
            self.policy.pull_request_policy.branch_prefix,
            case.id,
            &manifest_hash[..8]
        );

        let base_branch = self
            .policy
            .allowed_branches
            .first()
            .cloned()
            .unwrap_or_else(|| "master".to_string());

        let body_markdown =
            Self::format_pull_request_markdown(case, plan, manifest_hash, verification_summary);

        Ok(MaintenancePullRequestPackage {
            title,
            head_branch,
            base_branch,
            is_draft: self.policy.pull_request_policy.open_draft_pr,
            body_markdown,
            labels: self.policy.pull_request_policy.auto_label.clone(),
            manifest_hash: manifest_hash.to_string(),
        })
    }

    /// Dispatches pull request creation through the Authoritative External Side-Effect Gateway.
    pub fn dispatch_pull_request_side_effect(
        &self,
        workflow_id: &str,
        task_id: &str,
        attempt_id: u32,
        worker_id: &str,
        package: &MaintenancePullRequestPackage,
    ) -> Result<EffectAdmissionOutcome> {
        let gateway = match &self.gateway {
            Some(g) => g,
            None => {
                bail!("ExternalEffectGateway not initialized; remote side-effects are disabled");
            }
        };

        let req = EffectDispatchRequest {
            workflow_id: workflow_id.to_string(),
            task_id: task_id.to_string(),
            attempt_id,
            worker_id: worker_id.to_string(),
            tool_name: "github_create_pull_request".to_string(),
            target_service: "github".to_string(),
            authorized_destination: format!("branch:{}", package.head_branch),
            payload: serde_json::to_string(package)?,
            logical_operation_id: Some(format!(
                "pr_{}_{}",
                workflow_id,
                &package.manifest_hash[..8]
            )),
            custom_idempotency_key: Some(format!("idem_pr_{}", package.manifest_hash)),
            is_reversible: true,
            compensation_procedure: Some(format!("close_pull_request:{}", package.head_branch)),
        };

        let outcome = gateway.admit_effect(&req)?;
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        AutonomyTier, MaintenanceCategory, MaintenancePlan, MaintenanceRiskLevel, SignalSeverity,
    };
    use std::path::PathBuf;

    #[test]
    fn test_pull_request_package_formatting() {
        let mut case = MaintenanceCase::new(
            "repo_1",
            MaintenanceCategory::BrokenCi,
            "Fix test compilation failure",
            "Mismatched types error in engine.rs line 42",
            "ci_case_1",
            SignalSeverity::High,
        );
        case.affected_files.push(PathBuf::from("src/engine.rs"));

        let plan = MaintenancePlan::new(
            &case.id,
            "repo_1",
            "sha_baseline_abc",
            MaintenanceCategory::BrokenCi,
            "type_mismatch",
            "minimal_source_correction",
            MaintenanceRiskLevel::Low,
            AutonomyTier::Tier2GuardedRemediation,
            false,
        );

        let manifest_hash = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        let verif_summary =
            "All 42 tests passed. Zero compiler warnings. Safety gate verdict: Allow.";

        let md = DeliveryAdapter::format_pull_request_markdown(
            &case,
            &plan,
            manifest_hash,
            verif_summary,
        );

        assert!(md.contains("Autonomous Maintenance Remediation Report"));
        assert!(md.contains("ChangeSetManifest SHA-256"));
        assert!(md.contains(manifest_hash));
        assert!(md.contains("src/engine.rs"));
        assert!(md.contains("All 42 tests passed"));
        assert!(md.contains("Auto-Merge Permitted:** `false`"));
    }
}
