//! World state: named, diffable system-prompt sections (report §5.3,
//! upstream `core/src/context/world_state/`).
//!
//! The system prompt is assembled from *sections* with stable identities.
//! Each section's rendered bytes are digested with SHA-1; a rebuild diffs
//! digests instead of comparing raw strings, so callers (TUI refresh, future
//! mid-run goal updates) can tell exactly which pieces of context changed —
//! the prerequisite for emitting incremental updates instead of re-listing
//! everything.

use serde::Serialize;
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;

/// Where a section belongs in the assembled context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    /// Structural sections rendered at the very start of the prompt.
    Prefix,
    /// Sections that are emitted as their own message (reserved for
    /// incremental tool-context updates; §7.5).
    Standalone,
    /// Context sections that may be merged or summarized by compaction.
    Mergeable,
}

/// A named, renderable piece of the world state.
pub trait WorldStateSection {
    /// Stable section identity used for diffing and placement.
    const ID: &'static str;
    /// Rendered section content (may be empty — empty sections are dropped).
    fn render(&self) -> String;
    /// Where the section belongs.
    fn placement(&self) -> Placement;
}

/// One section that changed between two world-state builds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SectionDelta {
    pub id: String,
    pub kind: DeltaKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeltaKind {
    Added,
    Removed,
    Changed,
}

/// An ordered set of named world-state sections.
#[derive(Debug, Clone, Default)]
pub struct WorldState {
    sections: Vec<(String, String, Placement)>,
}

impl WorldState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a section from a [`WorldStateSection`] implementor.
    pub fn push_section<S: WorldStateSection>(&mut self, section: S) {
        let rendered = section.render();
        if rendered.is_empty() {
            return;
        }
        self.sections
            .push((S::ID.to_string(), rendered, section.placement()));
    }

    /// Adds a pre-rendered section (internal builder use).
    pub fn push_raw(&mut self, id: &str, rendered: String, placement: Placement) {
        if rendered.is_empty() {
            return;
        }
        self.sections.push((id.to_string(), rendered, placement));
    }

    /// Joins the sections in order — the assembled prompt text.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for (_, text, _) in &self.sections {
            out.push_str(text);
        }
        out
    }

    /// Ordered (id, placement) pairs — for structural introspection.
    pub fn section_ids(&self) -> Vec<(String, Placement)> {
        self.sections
            .iter()
            .map(|(id, _, p)| (id.clone(), *p))
            .collect()
    }

    /// SHA-1 digest (hex) of each rendered section.
    pub fn section_digests(&self) -> BTreeMap<String, String> {
        self.sections
            .iter()
            .map(|(id, text, _)| (id.clone(), sha1_hex(text)))
            .collect()
    }

    /// Diffs this state against the digests of a previous build.
    pub fn diff_since(&self, previous: &BTreeMap<String, String>) -> Vec<SectionDelta> {
        let current = self.section_digests();
        let mut deltas = Vec::new();
        for (id, digest) in &current {
            match previous.get(id) {
                None => deltas.push(SectionDelta {
                    id: id.clone(),
                    kind: DeltaKind::Added,
                }),
                Some(prev) if prev != digest => deltas.push(SectionDelta {
                    id: id.clone(),
                    kind: DeltaKind::Changed,
                }),
                Some(_) => {}
            }
        }
        for id in previous.keys() {
            if !current.contains_key(id) {
                deltas.push(SectionDelta {
                    id: id.clone(),
                    kind: DeltaKind::Removed,
                });
            }
        }
        deltas
    }
}

fn sha1_hex(text: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(text.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{b:02x}")).collect()
}

/// Public SHA-1 hex digest of arbitrary text. Shared by compaction
/// checkpoints (§5.5) for stable model-identity hashing.
pub fn digest_hex(text: &str) -> String {
    sha1_hex(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(&'static str);

    impl WorldStateSection for Fixed {
        const ID: &'static str = "fixed";
        fn render(&self) -> String {
            self.0.to_string()
        }
        fn placement(&self) -> Placement {
            Placement::Mergeable
        }
    }

    #[test]
    fn test_sha1_known_vector() {
        assert_eq!(sha1_hex("abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn test_diff_added_changed_removed() {
        let mut ws = WorldState::new();
        ws.push_section(Fixed("hello"));

        // No previous build → everything is new.
        let first = ws.diff_since(&BTreeMap::new());
        assert_eq!(
            first,
            vec![SectionDelta {
                id: "fixed".into(),
                kind: DeltaKind::Added
            }]
        );

        // Identical rebuild → empty diff.
        let d1 = ws.section_digests();
        assert!(ws.diff_since(&d1).is_empty());

        let mut ws2 = WorldState::new();
        ws2.push_section(Fixed("world"));
        let deltas = ws2.diff_since(&d1);
        assert_eq!(
            deltas,
            vec![SectionDelta {
                id: "fixed".into(),
                kind: DeltaKind::Changed
            }]
        );

        let mut ws3 = WorldState::new();
        ws3.push_raw("other", "text".into(), Placement::Standalone);
        let deltas = ws3.diff_since(&d1);
        assert_eq!(
            deltas,
            vec![
                SectionDelta {
                    id: "other".into(),
                    kind: DeltaKind::Added
                },
                SectionDelta {
                    id: "fixed".into(),
                    kind: DeltaKind::Removed
                },
            ]
        );
    }

    #[test]
    fn test_empty_sections_dropped_and_render_order() {
        let mut ws = WorldState::new();
        ws.push_raw("first", "A".into(), Placement::Prefix);
        ws.push_raw("empty", String::new(), Placement::Mergeable);
        ws.push_raw("second", "B".into(), Placement::Prefix);
        assert_eq!(ws.render(), "AB");
        assert_eq!(
            ws.section_ids(),
            vec![
                ("first".to_string(), Placement::Prefix),
                ("second".to_string(), Placement::Prefix),
            ]
        );
    }
}
