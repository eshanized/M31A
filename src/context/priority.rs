//! 5-class deterministic priority pipeline and reverse compaction engine (D-14, D-16, CTX-02, CTX-05).
//!
//! Priorities:
//! - P0 Runtime/Safety (unprunable)
//! - P1 Task/Criteria (unprunable)
//! - P2 Required Evidence (loss-preserving compaction)
//! - P3 Active Working Context (recent turn window / summaries)
//! - P4 Optional Background (discardable first)
//!
//! Enforces protected budget gate: if P0+P1 exceeds admissible budget, fails closed with ContextBudgetExceeded.

use serde::{Deserialize, Serialize};

use crate::context::envelope::TrustLevel;
use crate::context::manifest::ManifestSectionEntry;
use crate::context::tokenizer::TokenizerAdapter;
use crate::kernel::seams::context::ContextError;

/// 5 deterministic semantic priority classes (D-16, CTX-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextPriority {
    /// P0: Safety invariants, runtime policy constraints, role system instructions (unprunable).
    P0RuntimeSafety = 0,
    /// P1: Task description, acceptance criteria, verification strategy (unprunable).
    P1TaskCompletion = 1,
    /// P2: Prerequisite verified TaskResults, resolved artifact summaries (loss-preserving compaction).
    P2RequiredEvidence = 2,
    /// P3: Active working context, recent turns, intermediate step outputs (bounded sliding window).
    P3ActiveWorkingContext = 3,
    /// P4: Codebase enrichment, surrounding symbol topology, git history (discardable first).
    P4OptionalBackground = 4,
}

impl ContextPriority {
    pub fn rank(&self) -> u8 {
        *self as u8
    }

    pub fn is_protected(&self) -> bool {
        matches!(
            self,
            ContextPriority::P0RuntimeSafety | ContextPriority::P1TaskCompletion
        )
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ContextPriority::P0RuntimeSafety => "P0_runtime_safety",
            ContextPriority::P1TaskCompletion => "P1_task_completion",
            ContextPriority::P2RequiredEvidence => "P2_required_evidence",
            ContextPriority::P3ActiveWorkingContext => "P3_active_working_context",
            ContextPriority::P4OptionalBackground => "P4_optional_background",
        }
    }
}

impl std::fmt::Display for ContextPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A structured section of context with priority, provenance, and trust metadata (D-16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSection {
    pub id: String,
    pub priority: ContextPriority,
    pub content: String,
    pub token_count: usize,
    pub provenance: String,
    pub trust_level: TrustLevel,
    pub compactable: bool,
}

impl ContextSection {
    pub fn new(
        id: impl Into<String>,
        priority: ContextPriority,
        content: impl Into<String>,
        provenance: impl Into<String>,
        trust_level: TrustLevel,
        compactable: bool,
        tokenizer: &TokenizerAdapter,
    ) -> Self {
        let content_str = content.into();
        let tokens = tokenizer.count_tokens(&content_str);
        Self {
            id: id.into(),
            priority,
            content: content_str,
            token_count: tokens,
            provenance: provenance.into(),
            trust_level,
            compactable,
        }
    }
}

/// Outcome of running reverse-priority compaction on context sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionResult {
    pub compacted_sections: Vec<ContextSection>,
    pub manifest_entries: Vec<ManifestSectionEntry>,
    pub final_token_count: usize,
}

/// Deterministic reverse-priority compaction engine (D-14, D-16, CTX-05).
pub struct ReverseCompactor {
    tokenizer: TokenizerAdapter,
}

impl ReverseCompactor {
    pub fn new(tokenizer: TokenizerAdapter) -> Self {
        Self { tokenizer }
    }

    /// Compacts sections in reverse priority order (P4 -> P3 -> P2), failing closed if P0+P1 overflow budget.
    pub fn compact(
        &self,
        sections: Vec<ContextSection>,
        admissible_budget: usize,
    ) -> Result<CompactionResult, ContextError> {
        // 1. Protected Budget Gate: calculate P0 + P1 tokens (D-14, D-16, T-07-14)
        let mut p0_p1_tokens: usize = 0;
        for s in &sections {
            if s.priority.is_protected() {
                p0_p1_tokens = p0_p1_tokens.saturating_add(s.token_count);
            }
        }

        if p0_p1_tokens > admissible_budget {
            return Err(ContextError::CompilationFailed(format!(
                "ContextBudgetExceeded: protected context (P0 + P1) requires {} tokens, exceeding admissible budget {}",
                p0_p1_tokens, admissible_budget
            )));
        }

        // Calculate initial total
        let mut total_tokens: usize = sections.iter().map(|s| s.token_count).sum();

        // If everything fits as-is, retain full
        if total_tokens <= admissible_budget {
            let manifest_entries = sections
                .iter()
                .map(|s| ManifestSectionEntry {
                    section_id: s.id.clone(),
                    priority: s.priority,
                    original_tokens: s.token_count,
                    final_tokens: s.token_count,
                    compaction_action: "retained_full".to_string(),
                    provenance: s.provenance.clone(),
                })
                .collect();

            return Ok(CompactionResult {
                compacted_sections: sections,
                manifest_entries,
                final_token_count: total_tokens,
            });
        }

        // We need compaction. Process in reverse priority order:
        // We will maintain working mutable copies of each section and their original tokens
        let mut working_sections = sections;
        let mut actions: Vec<String> = vec!["retained_full".to_string(); working_sections.len()];
        let original_tokens: Vec<usize> = working_sections.iter().map(|s| s.token_count).collect();

        // Step 1: Drop P4 (Optional Background) sections (from last P4 to first P4)
        for i in (0..working_sections.len()).rev() {
            if working_sections[i].priority == ContextPriority::P4OptionalBackground {
                let saved = working_sections[i].token_count;
                working_sections[i].content.clear();
                working_sections[i].token_count = 0;
                actions[i] = "dropped".to_string();
                total_tokens = total_tokens.saturating_sub(saved);

                if total_tokens <= admissible_budget {
                    break;
                }
            }
        }

        // Step 2: Compact P3 (Active Working Context) if still over budget
        if total_tokens > admissible_budget {
            for i in (0..working_sections.len()).rev() {
                if working_sections[i].priority == ContextPriority::P3ActiveWorkingContext
                    && working_sections[i].compactable
                    && working_sections[i].token_count > 0
                {
                    let old_tokens = working_sections[i].token_count;
                    let summary = format!("[Turn Summary: {}]", working_sections[i].id);
                    let new_tokens = self.tokenizer.count_tokens(&summary);
                    if new_tokens < old_tokens {
                        working_sections[i].content = summary;
                        working_sections[i].token_count = new_tokens;
                        actions[i] = "compacted_summary".to_string();
                        total_tokens = total_tokens
                            .saturating_sub(old_tokens)
                            .saturating_add(new_tokens);
                    }
                    if total_tokens <= admissible_budget {
                        break;
                    }
                }
            }
        }

        // Step 3: Compact P2 (Required Evidence) via loss-preserving excerpts if still over budget
        if total_tokens > admissible_budget {
            for i in (0..working_sections.len()).rev() {
                if working_sections[i].priority == ContextPriority::P2RequiredEvidence
                    && working_sections[i].compactable
                    && working_sections[i].token_count > 0
                {
                    let old_tokens = working_sections[i].token_count;
                    let excerpt: String = working_sections[i]
                        .content
                        .lines()
                        .take(2)
                        .collect::<Vec<_>>()
                        .join("\n");
                    let formatted_excerpt = format!("{}\n... [excerpt]", excerpt);
                    let new_tokens = self.tokenizer.count_tokens(&formatted_excerpt);
                    if new_tokens < old_tokens {
                        working_sections[i].content = formatted_excerpt;
                        working_sections[i].token_count = new_tokens;
                        actions[i] = "compacted_excerpt".to_string();
                        total_tokens = total_tokens
                            .saturating_sub(old_tokens)
                            .saturating_add(new_tokens);
                    }
                    if total_tokens <= admissible_budget {
                        break;
                    }
                }
            }
        }

        // Step 4: Drop P3 completely if still over budget (preserving P2 evidence)
        if total_tokens > admissible_budget {
            for i in (0..working_sections.len()).rev() {
                if working_sections[i].priority == ContextPriority::P3ActiveWorkingContext {
                    let saved = working_sections[i].token_count;
                    working_sections[i].content.clear();
                    working_sections[i].token_count = 0;
                    actions[i] = "dropped".to_string();
                    total_tokens = total_tokens.saturating_sub(saved);
                    if total_tokens <= admissible_budget {
                        break;
                    }
                }
            }
        }

        // Final check: If after all compaction steps, total tokens still exceed budget, fail closed (CTX-05)
        if total_tokens > admissible_budget {
            return Err(ContextError::CompilationFailed(format!(
                "ContextBudgetExceeded: context requires {} tokens after all compaction, exceeding admissible budget {}",
                total_tokens, admissible_budget
            )));
        }

        // Retain only sections with non-empty content (or explicitly marked dropped in manifest)
        let mut final_sections = Vec::new();
        let mut manifest_entries = Vec::new();

        for (i, section) in working_sections.into_iter().enumerate() {
            manifest_entries.push(ManifestSectionEntry {
                section_id: section.id.clone(),
                priority: section.priority,
                original_tokens: original_tokens[i],
                final_tokens: section.token_count,
                compaction_action: actions[i].clone(),
                provenance: section.provenance.clone(),
            });

            if !section.content.is_empty() {
                final_sections.push(section);
            }
        }

        Ok(CompactionResult {
            compacted_sections: final_sections,
            manifest_entries,
            final_token_count: total_tokens,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_section(
        id: &str,
        priority: ContextPriority,
        content: &str,
        compactable: bool,
        tokenizer: &TokenizerAdapter,
    ) -> ContextSection {
        ContextSection::new(
            id,
            priority,
            content,
            format!("test://{}", id),
            TrustLevel::TrustedSystem,
            compactable,
            tokenizer,
        )
    }

    #[test]
    fn test_protected_budget_gate_fails_closed() {
        let tokenizer = TokenizerAdapter::conservative();
        let compactor = ReverseCompactor::new(tokenizer);

        // P0 (50 chars = ~15 tokens) + P1 (50 chars = ~15 tokens) = ~30 tokens
        // Budget = 20 tokens -> must fail closed
        let p0 = make_section(
            "p0",
            ContextPriority::P0RuntimeSafety,
            &"A".repeat(50),
            false,
            &compactor.tokenizer,
        );
        let p1 = make_section(
            "p1",
            ContextPriority::P1TaskCompletion,
            &"B".repeat(50),
            false,
            &compactor.tokenizer,
        );

        let err = compactor.compact(vec![p0, p1], 20).unwrap_err();
        match err {
            ContextError::CompilationFailed(msg) => {
                assert!(msg.contains("ContextBudgetExceeded"));
                assert!(msg.contains("protected context (P0 + P1)"));
            }
        }
    }

    #[test]
    fn test_reverse_compaction_drops_p4_before_p3() {
        let tokenizer = TokenizerAdapter::conservative();
        let compactor = ReverseCompactor::new(tokenizer);

        let p0 = make_section(
            "p0",
            ContextPriority::P0RuntimeSafety,
            "Safety rules",
            false,
            &compactor.tokenizer,
        );
        let p1 = make_section(
            "p1",
            ContextPriority::P1TaskCompletion,
            "Task criteria",
            false,
            &compactor.tokenizer,
        );
        let p3 = make_section(
            "p3",
            ContextPriority::P3ActiveWorkingContext,
            "Recent steps history line",
            true,
            &compactor.tokenizer,
        );
        let p4 = make_section(
            "p4",
            ContextPriority::P4OptionalBackground,
            "Large background docs enrichment",
            false,
            &compactor.tokenizer,
        );

        // Calculate tokens for P0 + P1 + P3
        let target_budget = p0.token_count + p1.token_count + p3.token_count + 2;

        let res = compactor
            .compact(vec![p0, p1, p3, p4], target_budget)
            .unwrap();

        // P4 should be dropped
        let p4_entry = res
            .manifest_entries
            .iter()
            .find(|e| e.section_id == "p4")
            .unwrap();
        assert_eq!(p4_entry.compaction_action, "dropped");
        assert_eq!(p4_entry.final_tokens, 0);

        // P3 should be retained full since budget allowed it after dropping P4
        let p3_entry = res
            .manifest_entries
            .iter()
            .find(|e| e.section_id == "p3")
            .unwrap();
        assert_eq!(p3_entry.compaction_action, "retained_full");

        // Sections list should only have p0, p1, p3
        assert_eq!(res.compacted_sections.len(), 3);
        assert!(!res.compacted_sections.iter().any(|s| s.id == "p4"));
    }

    #[test]
    fn test_reverse_compaction_compacts_p3_and_p2_under_tight_budget() {
        let tokenizer = TokenizerAdapter::conservative();
        let compactor = ReverseCompactor::new(tokenizer);

        let p0 = make_section(
            "p0",
            ContextPriority::P0RuntimeSafety,
            "Safety",
            false,
            &compactor.tokenizer,
        );
        let p1 = make_section(
            "p1",
            ContextPriority::P1TaskCompletion,
            "Criteria",
            false,
            &compactor.tokenizer,
        );
        let p2 = make_section(
            "p2",
            ContextPriority::P2RequiredEvidence,
            "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8",
            true,
            &compactor.tokenizer,
        );
        let p3 = make_section(
            "p3",
            ContextPriority::P3ActiveWorkingContext,
            "Step 1 executed\nStep 2 executed\nStep 3 executed",
            true,
            &compactor.tokenizer,
        );

        let tight_budget = p0.token_count + p1.token_count + 25;
        let res = compactor
            .compact(vec![p0, p1, p2, p3], tight_budget)
            .unwrap();

        assert!(res.final_token_count <= tight_budget);
        let p3_entry = res
            .manifest_entries
            .iter()
            .find(|e| e.section_id == "p3")
            .unwrap();
        assert_eq!(p3_entry.compaction_action, "compacted_summary");
    }
}
