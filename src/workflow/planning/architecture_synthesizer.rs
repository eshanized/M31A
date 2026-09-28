//! System Architecture Synthesizer extracting topologies, interfaces, and candidate ADRs.

use super::adr::{AdrId, AdrRegistry, ArchitectureDecisionRecord};
use super::architecture::{
    ArchitectureComponent, ArchitectureDataStore, ArchitectureDocument, ArchitectureInterface,
    ArchitectureSubsystem, ComponentStatus, DeploymentBoundary, TrustBoundary,
};
use super::decision::DecisionStatus;
use super::requirements::RequirementsDocument;
use crate::planning::requirements::{
    Provenance, ProvenanceSourceType, RequirementCategory, TrustLevel,
};
use crate::workflow::error::WorkflowError;
use crate::workflow::genesis::brownfield::BrownfieldMap;
use crate::workflow::genesis::dimension_registry::ResearchDimensionRegistry;
use crate::workflow::genesis::project::ProjectCharter;
use crate::workflow::genesis::provenance::ResearchFinding;
use crate::workflow::genesis::synthesis::ResearchSummary;
use sha2::{Digest, Sha256};

/// Autonomous synthesizer deriving formal architecture topologies and candidate ADRs.
pub struct ArchitectureSynthesizer;

impl ArchitectureSynthesizer {
    /// Synthesize system architecture and candidate ADRs from requirements, charter, and research.
    pub fn synthesize(
        requirements: &RequirementsDocument,
        charter: &ProjectCharter,
        summary: Option<&ResearchSummary>,
        findings: &[ResearchFinding],
        brownfield: Option<&BrownfieldMap>,
    ) -> Result<(ArchitectureDocument, AdrRegistry), WorkflowError> {
        let mut arch = ArchitectureDocument::new(
            &charter.project_name,
            format!(
                "System architecture specification for project '{}', implementing {} requirements.",
                charter.project_name,
                requirements.requirements.len()
            ),
        );

        arch.architectural_goals.push(format!(
            "Deliver '{}' within bounded resource envelopes",
            charter.project_name
        ));
        arch.architectural_goals
            .push("Maintain strict decoupling between policy/security and execution".to_string());
        arch.architectural_goals
            .push("All long-running operations must be cancellable and verifiable".to_string());

        let mut adr_registry = AdrRegistry::new();
        let mut adr_num = 1;

        // 1. Subsystems & Components Synthesis
        //
        // Every identity is derived from target evidence
        // (scanned modules, delta scope, requirement keys, finding topics).
        // No universal SUB-*/CMP-* table is consulted. The EXISTING/DELTA
        // markers describe the brownfield relationship (a generic mechanism),
        // while the hash suffix binds each identity to actual target content.
        // Component trackers below feed trust/persistence derivation; they
        // carry ids, never fixed table positions.
        let mut brownfield_delta_comp: Option<String> = None;
        let mut operational_component: Option<String> = None;
        let mut security_components: Vec<String> = Vec::new();
        if let Some(bm) = brownfield {
            // Brownfield Mode: pre-existing modules become Existing
            // components named after the scanned module paths; the delta
            // becomes one New subsystem bound to the delta requirement keys.
            let mut sorted_modules = bm.topology.module_tree.clone();
            sorted_modules.sort();
            let existing_hash =
                content_hash(&[charter.project_name.as_str(), &sorted_modules.join("\n")]);
            let existing_sub_id = format!("SUB-EXISTING-{existing_hash}");
            let mut existing_sub = ArchitectureSubsystem::new(
                &existing_sub_id,
                format!("Existing {} Codebase", bm.topology.primary_language),
                "Pre-existing codebase components discovered during brownfield scanning",
            );

            for (idx, module) in bm.topology.module_tree.iter().enumerate() {
                let comp_id = format!("CMP-{}-{:02}", slugify(module), idx + 1);
                let comp = ArchitectureComponent::new(
                    &comp_id,
                    format!("Module: {}", module),
                    &existing_sub_id,
                    format!("Pre-existing legacy component for {}", module),
                )
                .with_status(ComponentStatus::Existing);

                existing_sub.components.push(comp_id);
                arch.add_component(comp)?;
            }
            arch.subsystems.push(existing_sub);

            // New / Delta Subsystem
            let delta_name = bm.delta_scope.as_deref().unwrap_or("Extension Subsystem");
            let mut delta_key_strings: Vec<String> = requirements
                .requirements
                .iter()
                .map(|r| r.key.to_string())
                .collect();
            delta_key_strings.sort();
            let delta_hash = content_hash(&[
                charter.project_name.as_str(),
                delta_name,
                &delta_key_strings.join("\n"),
            ]);
            let delta_sub_id = format!("SUB-DELTA-{delta_hash}");
            let mut delta_sub = ArchitectureSubsystem::new(
                &delta_sub_id,
                delta_name,
                format!("Target extension subsystem: {}", delta_name),
            );

            let delta_comp_id = format!("CMP-DELTA-{delta_hash}");
            let delta_reqs = requirements
                .requirements
                .iter()
                .map(|r| r.key.clone())
                .collect();
            let delta_comp = ArchitectureComponent::new(
                &delta_comp_id,
                delta_name,
                &delta_sub_id,
                format!("New subsystem logic: {}", delta_name),
            )
            .with_requirements(delta_reqs)
            .with_status(ComponentStatus::New);

            delta_sub.components.push(delta_comp_id.clone());
            arch.subsystems.push(delta_sub);
            arch.add_component(delta_comp)?;
            brownfield_delta_comp = Some(delta_comp_id);
        } else {
            // Greenfield Mode: derive subsystem topology from requirement
            // evidence and research findings. Groups materialize only for
            // requirement categories (or research dimensions) actually
            // present; subsystem/component identities derive from the
            // project name plus member requirement keys and finding topics,
            // so an unseen domain yields its own identities with zero
            // source changes.
            let groups = derive_requirement_groups(&charter.project_name, requirements, findings)?;
            let mut previous_component: Option<String> = None;
            for group in &groups {
                let mut subsystem = ArchitectureSubsystem::new(
                    group.subsystem_id.clone(),
                    format!("{} {}", charter.project_name, group.label),
                    format!(
                        "Derived {} subsystem for {} covering {} requirement(s)",
                        group.label,
                        charter.project_name,
                        group.requirement_keys.len()
                    ),
                );

                let mut responsibility = format!(
                    "{} {} responsibilities for {}",
                    charter.project_name, group.label, charter.project_name
                );
                let titles: Vec<String> =
                    group.requirement_titles.iter().take(3).cloned().collect();
                if !titles.is_empty() {
                    responsibility.push_str(&format!(": {}", titles.join("; ")));
                }
                if group.category == RequirementCategory::Functional
                    && !charter.domain_model.entities.is_empty()
                {
                    responsibility.push_str(&format!(
                        ". Manages domain entities: {}",
                        charter.domain_model.entities.join(", ")
                    ));
                }
                if !group.finding_topics.is_empty() {
                    responsibility.push_str(&format!(
                        ". Research evidence: {}",
                        group.finding_topics.join("; ")
                    ));
                }

                let mut component = ArchitectureComponent::new(
                    group.component_id.clone(),
                    format!("{} {}", charter.project_name, group.label),
                    group.subsystem_id.clone(),
                    responsibility,
                )
                .with_requirements(group.requirement_keys.clone())
                .with_status(ComponentStatus::New);
                if let Some(prev) = previous_component.clone() {
                    component = component.with_depends_on(vec![prev]);
                }

                subsystem.components.push(group.component_id.clone());
                arch.subsystems.push(subsystem);
                arch.add_component(component)?;
                if group.category == RequirementCategory::Operational
                    && operational_component.is_none()
                {
                    operational_component = Some(group.component_id.clone());
                }
                if group.category == RequirementCategory::Security
                    || group.category == RequirementCategory::Compliance
                {
                    security_components.push(group.component_id.clone());
                }
                previous_component = Some(group.component_id.clone());
            }
        }

        // 2. Interfaces: derived from component dependency edges. The
        // brownfield delta/host contract references the derived delta
        // component; its contract type is a neutral hook label, never a
        // presupposed host language.
        if brownfield.is_some() {
            let delta_comp_id = brownfield_delta_comp.clone().ok_or_else(|| {
                WorkflowError::InvalidDefinition(
                    "brownfield delta component missing after synthesis".to_string(),
                )
            })?;
            arch.interfaces.push(ArchitectureInterface::new(
                format!("IFACE-{delta_comp_id}-HOOK"),
                "Extension Hook Contract",
                delta_comp_id,
                "ExtensionHook",
                "Integration interface connecting extension subsystem with pre-existing modules",
            ));
        } else {
            let ordered: Vec<String> = arch.components.iter().map(|c| c.id.clone()).collect();
            for window in ordered.windows(2) {
                let (provider, consumer) = (window[0].clone(), window[1].clone());
                let mut iface = ArchitectureInterface::new(
                    format!("IFACE-{}-CONTRACT", consumer),
                    format!("Derived contract {} -> {}", provider, consumer),
                    provider.clone(),
                    "InternalContract",
                    format!(
                        "Derived dependency contract: {} provides services consumed by {}",
                        provider, consumer
                    ),
                );
                iface.consumer_components = vec![consumer];
                arch.interfaces.push(iface);
            }
        }

        // 3. Persistence & Data Stores: derived from charter storage evidence.
        // No storage technology is presupposed; an undecided selection is
        // recorded as an open design question, never silently defaulted.
        // The host-filesystem store below is factual (a brownfield workspace
        // exists by definition), not a universal technology imposition.
        if brownfield.is_some() {
            let delta_comp_id = brownfield_delta_comp.clone().ok_or_else(|| {
                WorkflowError::InvalidDefinition(
                    "brownfield delta component missing after synthesis".to_string(),
                )
            })?;
            arch.persistence_model =
                "Host codebase filesystem and configuration repositories".to_string();
            arch.data_stores.push(ArchitectureDataStore {
                id: "DS-HOST".to_string(),
                name: "Host Repository Store".to_string(),
                store_type: "LocalFilesystem".to_string(),
                persistence_guarantees: "Atomic filesystem writes".to_string(),
                owning_component: delta_comp_id,
            });
        } else if let Some(storage) = charter.technical_preferences.storage.clone() {
            arch.persistence_model = storage.clone();
            // Ownership prefers the operations-category component when the
            // evidence produced one, else the foundation component. When the
            // architecture is empty the store is deferred honestly instead
            // of pointing at a fabricated owner.
            let owner = operational_component
                .clone()
                .or_else(|| arch.components.first().map(|c| c.id.clone()));
            if let Some(owner) = owner {
                let store_hash = content_hash(&[storage.as_str(), owner.as_str()]);
                arch.data_stores.push(ArchitectureDataStore {
                    id: format!("DS-{}-{store_hash}", slugify(&charter.project_name)),
                    name: format!("{} Primary Store", charter.project_name),
                    store_type: storage,
                    persistence_guarantees:
                        "Durable transactional persistence per project storage selection"
                            .to_string(),
                    owning_component: owner,
                });
            } else {
                arch.unresolved_design_questions.push(
                    "Storage evidence exists but the derived architecture has no owning component; resolve once requirements produce subsystems"
                        .to_string(),
                );
            }
        } else {
            arch.persistence_model =
                "Undecided: storage selection deferred pending persistence evidence".to_string();
            arch.unresolved_design_questions.push(
                "Select durable storage technology from persistence evidence gathered during research or implementation planning"
                    .to_string(),
            );
        }

        // 4. Trust Boundaries: derived from the emitted components. Components
        // of security/compliance evidence groups form the trusted core;
        // otherwise the foundation component does. The boundary id binds to
        // the inside-component set, never to a fixed table.
        let (inside_comps, outside_comps) = if brownfield.is_some() {
            (
                arch.components.iter().map(|c| c.id.clone()).collect(),
                vec!["ExternalHostEnvironment".to_string()],
            )
        } else {
            let inside = if security_components.is_empty() {
                arch.components
                    .first()
                    .map(|c| vec![c.id.clone()])
                    .unwrap_or_default()
            } else {
                security_components.clone()
            };
            let mut outside: Vec<String> = arch
                .components
                .iter()
                .map(|c| c.id.clone())
                .filter(|id| !inside.contains(id))
                .collect();
            outside.push("ExternalClients".to_string());
            (inside, outside)
        };

        if !inside_comps.is_empty() {
            let mut sorted_inside = inside_comps.clone();
            sorted_inside.sort();
            let boundary_hash = content_hash(&[sorted_inside.join("\n").as_str()]);
            arch.trust_boundaries.push(TrustBoundary {
                id: format!("TB-{boundary_hash}"),
                name: format!("{} Trust Boundary", charter.project_name),
                description:
                    "Derived trusted execution domain owning domain state mutation and business invariants"
                        .to_string(),
                inside_components: inside_comps,
                outside_components: outside_comps,
                boundary_controls: vec![
                    "All external client inputs validated against strict schemas".to_string(),
                    "Capability and permission checks for all mutating operations".to_string(),
                    "No raw execution of untrusted external scripts or commands".to_string(),
                ],
                authentication_required: true,
                authorization_rules: vec![
                    "Domain mutations must pass business invariant validation".to_string(),
                    "Mutations require transactional ownership lock".to_string(),
                ],
            });
        }

        // 5. Deployment Topology: derived from charter deployment evidence.
        let deploy_target = charter
            .technical_preferences
            .deployment_target
            .clone()
            .unwrap_or_else(|| {
                "Unspecified: single deployable unit assumed pending evidence".to_string()
            });
        let resource_bounds = if charter.operational_invariants.scale_targets.is_empty() {
            Some("Bounded execution: memory and CPU constrained by runtime budget".to_string())
        } else {
            Some(charter.operational_invariants.scale_targets.join("; "))
        };
        let mut deploy_inputs: Vec<String> = arch.components.iter().map(|c| c.id.clone()).collect();
        deploy_inputs.sort();
        deploy_inputs.insert(0, deploy_target.clone());
        let deploy_hash = content_hash(
            &deploy_inputs
                .iter()
                .map(String::as_str)
                .collect::<Vec<&str>>(),
        );
        arch.deployment_boundaries.push(DeploymentBoundary {
            id: format!("DEP-{deploy_hash}"),
            name: format!("{} Deployment Boundary", charter.project_name),
            target: deploy_target,
            resource_bounds,
            components: arch.components.iter().map(|c| c.id.clone()).collect(),
        });

        // 6. Data Flows & Recovery: flow follows the derived dependency chain
        // in both modes (brownfield order is scan order: host modules first,
        // then the delta component).
        {
            let chain: Vec<String> = arch.components.iter().map(|c| c.id.clone()).collect();
            if !chain.is_empty() {
                arch.data_flow
                    .push(format!("ExternalClients -> {}", chain.join(" -> ")));
            }
        }
        arch.failure_recovery_model =
            "Fail-fast with bounded retries, durable checkpoints, and non-destructive rollbacks"
                .to_string();

        for inv in &charter.operational_invariants.security_requirements {
            arch.architectural_invariants.push(inv.clone());
        }
        for perf in &charter.operational_invariants.performance_targets {
            arch.architectural_invariants.push(perf.clone());
        }

        // 7. Generate Candidate ADRs from derived topology, storage evidence,
        // research tradeoffs, and individual research findings (findings
        // genuinely propagate: each finding becomes a Proposed ADR linked to
        // its requirement group).
        let prov = Provenance::new(
            ProvenanceSourceType::RepositoryFile,
            TrustLevel::VerifiedRepository,
            "architecture_synthesizer",
        )
        .with_location("ARCHITECTURE.md");

        // ADR 1: Derived System Topology
        let adr1_id = AdrId::from_number(adr_num);
        adr_num += 1;
        let derived_subsystems: Vec<String> =
            arch.subsystems.iter().map(|s| s.id.clone()).collect();
        let dependency_chain: Vec<String> = arch.components.iter().map(|c| c.id.clone()).collect();
        let mut adr1 = ArchitectureDecisionRecord::new(
            adr1_id.clone(),
            "Derived Subsystem Topology",
            format!(
                "The project '{}' requires {} derived subsystem(s) ({}) covering {} requirement(s).",
                charter.project_name,
                derived_subsystems.len(),
                derived_subsystems.join(", "),
                requirements.requirements.len()
            ),
            format!(
                "Adopt derived subsystem topology with strict downward dependency flow ({}).",
                dependency_chain.join(" -> ")
            ),
            "Subsystem boundaries follow requirement and research evidence; interfaces are explicit and validated by quality gates.",
            prov.clone(),
        )
        .with_alternatives(vec![
            "Single-layer implementation (rejected: no explicit module boundaries)".to_string(),
            "Premature service decomposition (rejected: no distribution evidence)".to_string(),
        ])
        .with_consequences(vec![
            "Each subsystem owns explicit requirement references and interfaces".to_string(),
            "New subsystems require evidence (requirements or research findings)".to_string(),
        ])
        .with_status(DecisionStatus::Proposed);

        let all_req_keys: Vec<_> = requirements
            .requirements
            .iter()
            .map(|r| r.key.clone())
            .collect();
        adr1.linked_requirements = all_req_keys;
        arch.adr_refs.push(adr1_id.to_string());
        adr_registry.register(adr1)?;

        // ADR 2: Persistence Model (evidence-derived; honest when undecided)
        let adr2_id = AdrId::from_number(adr_num);
        adr_num += 1;
        let adr2 = match charter.technical_preferences.storage.clone() {
            Some(storage_choice) => ArchitectureDecisionRecord::new(
                adr2_id.clone(),
                "Durable Storage Engine Selection",
                format!(
                    "The project '{}' requires durable persistence aligned with its domain scale.",
                    charter.project_name
                ),
                format!(
                    "Adopt {} for primary transactional records.",
                    storage_choice
                ),
                "Provides required durability guarantees tailored to target operational scale.",
                prov.clone(),
            )
            .with_alternatives(vec![
                "Filesystem-backed records without transactions (rejected: corruption risk under concurrency)"
                    .to_string(),
            ])
            .with_consequences(vec![
                "Schema migrations must be managed deterministically".to_string(),
                "Storage access must be abstracted behind repository contracts".to_string(),
            ])
            .with_status(DecisionStatus::Proposed),
            None => ArchitectureDecisionRecord::new(
                adr2_id.clone(),
                "Durable Storage Selection (Open)",
                "No storage evidence was recorded in the charter or research synthesis.".to_string(),
                "Defer storage selection until persistence evidence is gathered during implementation planning."
                    .to_string(),
                "Avoids imposing an unevidenced storage technology on the target project.".to_string(),
                prov.clone(),
            )
            .with_status(DecisionStatus::Proposed),
        };

        arch.adr_refs.push(adr2_id.to_string());
        adr_registry.register(adr2)?;

        // ADR 3: Tradeoffs from Research Summary
        if let Some(sum) = summary {
            for to in &sum.tradeoffs {
                let adr_id = AdrId::from_number(adr_num);
                adr_num += 1;

                let adr = ArchitectureDecisionRecord::new(
                    adr_id.clone(),
                    format!("Tradeoff: {}", to.tradeoff_axis),
                    format!("Balancing options along axis '{}'", to.tradeoff_axis),
                    to.decision.clone(),
                    to.rationale.clone(),
                    prov.clone(),
                )
                .with_status(DecisionStatus::Proposed);

                arch.adr_refs.push(adr_id.to_string());
                adr_registry.register(adr)?;
            }
        }

        // ADR 4+: Individual research findings become Proposed ADRs so that
        // actual research evidence reaches the architecture artifact.
        for group in derive_requirement_groups(&charter.project_name, requirements, findings)?
            .iter()
            .filter(|g| !g.findings.is_empty())
        {
            for finding in &group.findings {
                let adr_id = AdrId::from_number(adr_num);
                adr_num += 1;
                let decision = finding
                    .recommendations
                    .first()
                    .cloned()
                    .unwrap_or_else(|| finding.summary.clone());
                let rationale = if finding.tradeoffs.is_empty() {
                    format!("Evidence from {} research dimension", finding.dimension)
                } else {
                    finding.tradeoffs.join("; ")
                };
                let mut adr = ArchitectureDecisionRecord::new(
                    adr_id.clone(),
                    format!("Research evidence: {}", finding.topic),
                    finding.summary.clone(),
                    decision,
                    rationale,
                    prov.clone(),
                )
                .with_status(DecisionStatus::Proposed);
                adr.linked_requirements = group.requirement_keys.clone();
                arch.adr_refs.push(adr_id.to_string());
                adr_registry.register(adr)?;
            }
        }

        arch.sort_deterministic();
        Ok((arch, adr_registry))
    }
}

/// Slugify free text into an architecture-id fragment: uppercase
/// alphanumeric runs joined by single `-`, truncated to 20 chars, never
/// empty. Deterministic: identical input always yields identical output.
fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
        if out.len() >= 20 {
            break;
        }
    }
    let slug = out.trim_matches('-').to_string();
    if slug.is_empty() {
        "X".to_string()
    } else {
        slug
    }
}

/// Stable 6-hex content hash binding a derived identity to target evidence.
/// Identical evidence always yields an identical id; distinct evidence
/// overwhelmingly yields distinct ids.
fn content_hash(inputs: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for input in inputs {
        hasher.update(input.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())[..6].to_string()
}

/// Presentation label for a requirement category (`functional` → `Functional`).
fn category_label(category: RequirementCategory) -> String {
    let display = category.to_string();
    let mut chars = display.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => "General".to_string(),
    }
}

/// A derived architecture group: one subsystem/component pair computed from
/// requirement evidence (never emitted unconditionally).
///
/// Identities derive from the project name plus member requirement keys and
/// finding topics — never from a fixed architecture table.
struct DerivedGroup {
    category: RequirementCategory,
    subsystem_id: String,
    component_id: String,
    label: String,
    requirement_keys: Vec<crate::planning::requirements::RequirementKey>,
    requirement_titles: Vec<String>,
    finding_topics: Vec<String>,
    findings: Vec<ResearchFinding>,
}

/// Derive architecture groups from the requirement categories present plus
/// research finding dimensions. Deterministic: groups follow requirement-
/// category order and requirements keep document order within a group.
///
/// Findings join the group of their dimension's declared requirement
/// category (registry data). An unregistered dimension id fails explicitly:
/// unattributed research must never silently vanish from the architecture.
fn derive_requirement_groups(
    project_name: &str,
    requirements: &RequirementsDocument,
    findings: &[ResearchFinding],
) -> Result<Vec<DerivedGroup>, WorkflowError> {
    use std::collections::BTreeMap;
    let mut keys: BTreeMap<
        RequirementCategory,
        Vec<crate::planning::requirements::RequirementKey>,
    > = BTreeMap::new();
    let mut titles: BTreeMap<RequirementCategory, Vec<String>> = BTreeMap::new();
    for req in &requirements.requirements {
        keys.entry(req.category).or_default().push(req.key.clone());
        titles
            .entry(req.category)
            .or_default()
            .push(req.title_str().to_string());
    }

    let dim_registry = ResearchDimensionRegistry::global().read().map_err(|_| {
        WorkflowError::InvalidDefinition("dimension registry lock poisoned".to_string())
    })?;
    let mut group_findings: BTreeMap<RequirementCategory, Vec<ResearchFinding>> = BTreeMap::new();
    for finding in findings {
        let def = dim_registry.resolve(&finding.dimension).ok_or_else(|| {
            WorkflowError::InvalidDefinition(format!(
                "unknown research dimension '{}': finding cannot join the derived architecture without a registered dimension definition",
                finding.dimension.as_str()
            ))
        })?;
        group_findings
            .entry(def.requirement_category)
            .or_default()
            .push(finding.clone());
    }
    drop(dim_registry);

    let mut categories: Vec<RequirementCategory> = keys
        .keys()
        .chain(group_findings.keys())
        .cloned()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    categories.sort();

    let mut groups = Vec::new();
    for category in categories {
        let req_keys = keys.remove(&category).unwrap_or_default();
        let req_titles = titles.remove(&category).unwrap_or_default();
        let findings = group_findings.remove(&category).unwrap_or_default();
        if req_keys.is_empty() && findings.is_empty() {
            continue;
        }
        let mut owned: Vec<String> = vec![project_name.to_string(), category.to_string()];
        owned.extend(req_keys.iter().map(|k| k.to_string()));
        owned.extend(findings.iter().map(|f| f.topic.clone()));
        owned.sort();
        let refs: Vec<&str> = owned.iter().map(String::as_str).collect();
        let hash = content_hash(&refs);
        let slug = slugify(&category.to_string().replace('_', " "));
        let label = category_label(category);
        groups.push(DerivedGroup {
            category,
            subsystem_id: format!("SUB-{slug}-{hash}"),
            component_id: format!("CMP-{slug}-{hash}"),
            label,
            requirement_keys: req_keys,
            requirement_titles: req_titles,
            finding_topics: findings.iter().map(|f| f.topic.clone()).collect(),
            findings,
        });
    }
    // Category order is deterministic (RequirementCategory Ord); the
    // execution chain built downstream follows component dependencies.
    Ok(groups)
}
