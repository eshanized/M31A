//! Cascading planning invalidation and change impact analysis (Section 31, 32, 33).

use super::adr::{AdrId, AdrRegistry};
use super::architecture::{ArchitectureDocument, ComponentStatus};
use super::decision::DecisionStatus;
use super::requirements::RequirementsDocument;
use super::roadmap::{Roadmap, RoadmapPhaseStatus};
use super::state::{PlanningLifecycleState, PlanningState};
use crate::planning::requirements::RequirementKey;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Typed change impact analysis changeset adhering to Section 33.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PlanningChangeSet {
    pub added_requirements: Vec<RequirementKey>,
    pub changed_requirements: Vec<RequirementKey>,
    pub removed_requirements: Vec<RequirementKey>,
    pub affected_architecture_components: Vec<String>,
    pub affected_adrs: Vec<AdrId>,
    pub affected_phases: Vec<String>,
}

impl PlanningChangeSet {
    /// True if the changeset contains any upstream requirement or downstream artifact mutations.
    pub fn has_changes(&self) -> bool {
        !self.added_requirements.is_empty()
            || !self.changed_requirements.is_empty()
            || !self.removed_requirements.is_empty()
            || !self.affected_architecture_components.is_empty()
            || !self.affected_adrs.is_empty()
            || !self.affected_phases.is_empty()
    }
}

/// Cascading invalidator evaluating dependency closures across planning artifacts.
pub struct PlanningInvalidator;

impl PlanningInvalidator {
    /// Compute change impact by comparing an older requirement version with a new requirement version.
    pub fn compute_changeset(
        old_reqs: &RequirementsDocument,
        new_reqs: &RequirementsDocument,
        arch: &ArchitectureDocument,
        adrs: &AdrRegistry,
        roadmap: &Roadmap,
    ) -> PlanningChangeSet {
        let mut changeset = PlanningChangeSet::default();

        let old_map: HashMap<_, _> = old_reqs
            .requirements
            .iter()
            .map(|r| (r.key.clone(), r))
            .collect();
        let new_map: HashMap<_, _> = new_reqs
            .requirements
            .iter()
            .map(|r| (r.key.clone(), r))
            .collect();

        // 1. Detect added and changed requirements
        for (key, new_r) in &new_map {
            if let Some(old_r) = old_map.get(key) {
                // Check if semantically changed
                if old_r.statement() != new_r.statement()
                    || old_r.priority != new_r.priority
                    || old_r.category != new_r.category
                    || old_r.satisfaction_criteria != new_r.satisfaction_criteria
                {
                    changeset.changed_requirements.push(key.clone());
                }
            } else {
                changeset.added_requirements.push(key.clone());
            }
        }

        // 2. Detect removed requirements
        for key in old_map.keys() {
            if !new_map.contains_key(key) {
                changeset.removed_requirements.push(key.clone());
            }
        }

        // Collect all directly impacted requirement keys
        let impacted_keys: HashSet<_> = changeset
            .changed_requirements
            .iter()
            .chain(changeset.removed_requirements.iter())
            .cloned()
            .collect();

        if impacted_keys.is_empty() && changeset.added_requirements.is_empty() {
            return changeset;
        }

        // 3. Trace affected architecture components
        let mut affected_comps = HashSet::new();
        for comp in &arch.components {
            if comp
                .requirement_refs
                .iter()
                .any(|r| impacted_keys.contains(r))
            {
                affected_comps.insert(comp.id.clone());
            }
        }
        changeset.affected_architecture_components = affected_comps.into_iter().collect();
        changeset.affected_architecture_components.sort();

        // 4. Trace affected ADRs
        let mut affected_adrs = HashSet::new();
        for adr in adrs.adrs.values() {
            if adr
                .linked_requirements
                .iter()
                .any(|r| impacted_keys.contains(r))
            {
                affected_adrs.insert(adr.id.clone());
            }
        }
        changeset.affected_adrs = affected_adrs.into_iter().collect();
        changeset.affected_adrs.sort();

        // 5. Trace affected roadmap phases
        let affected_comp_set: HashSet<_> = changeset
            .affected_architecture_components
            .iter()
            .cloned()
            .collect();

        let mut affected_phases = HashSet::new();
        for phase in &roadmap.phases {
            if phase
                .requirement_refs
                .iter()
                .any(|r| impacted_keys.contains(r))
                || phase
                    .architecture_refs
                    .iter()
                    .any(|c| affected_comp_set.contains(c))
            {
                affected_phases.insert(phase.id.clone());
            }
        }
        changeset.affected_phases = affected_phases.into_iter().collect();
        changeset.affected_phases.sort();

        changeset
    }

    /// Apply cascading invalidation to architecture, ADRs, roadmap, and planning state.
    pub fn apply_invalidation(
        changeset: &PlanningChangeSet,
        arch: &mut ArchitectureDocument,
        adrs: &mut AdrRegistry,
        roadmap: &mut Roadmap,
        state: &mut PlanningState,
    ) {
        if !changeset.has_changes() {
            return;
        }

        // Invalidate affected architecture components
        for comp in &mut arch.components {
            if changeset
                .affected_architecture_components
                .contains(&comp.id)
            {
                comp.status = ComponentStatus::Modified;
            }
        }

        // Invalidate affected ADRs
        for adr_id in &changeset.affected_adrs {
            if let Some(adr) = adrs
                .get_mut(adr_id)
                .filter(|a| a.status == DecisionStatus::Accepted)
            {
                adr.status = DecisionStatus::NeedsOperatorDecision;
            }
        }

        // Invalidate affected roadmap phases
        for phase in &mut roadmap.phases {
            if changeset.affected_phases.contains(&phase.id) {
                phase.status = RoadmapPhaseStatus::Superseded;
            }
        }

        // Update planning state
        state.planning_version += 1;
        state.lifecycle_state = PlanningLifecycleState::RequirementsDraft;
        state.approval_status = "Invalidated due to upstream requirement mutation".to_string();

        for comp_id in &changeset.affected_architecture_components {
            let item = format!("Component '{}'", comp_id);
            if !state.invalidated_artifacts.contains(&item) {
                state.invalidated_artifacts.push(item);
            }
        }
        for adr_id in &changeset.affected_adrs {
            let item = format!("ADR '{}'", adr_id);
            if !state.invalidated_artifacts.contains(&item) {
                state.invalidated_artifacts.push(item);
            }
        }
        for phase_id in &changeset.affected_phases {
            let item = format!("Roadmap Phase '{}'", phase_id);
            if !state.invalidated_artifacts.contains(&item) {
                state.invalidated_artifacts.push(item);
            }
        }
    }
}
