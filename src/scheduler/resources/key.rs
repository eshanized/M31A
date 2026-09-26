use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ResourceNamespace {
    Workspace,
    Repository,
    Subsystem,
    Capability,
    Tool,
    Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PathScope {
    Exact,
    Subtree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LockMode {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResourceKey {
    pub namespace: ResourceNamespace,
    pub path_components: Vec<String>,
    pub scope: PathScope,
    pub canonical_id: String,
}

impl std::fmt::Display for ResourceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.canonical_id)
    }
}

impl ResourceKey {
    /// Create a workspace path resource key with component-wise normalization.
    pub fn workspace(path: impl AsRef<Path>, scope: PathScope) -> Self {
        let path_components: Vec<String> = path
            .as_ref()
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => s.to_str().map(|s| s.to_string()),
                _ => None,
            })
            .collect();

        let canonical_id = if path_components.is_empty() {
            match scope {
                PathScope::Subtree => "workspace:**".to_string(),
                PathScope::Exact => "workspace:".to_string(),
            }
        } else {
            let joined = path_components.join("/");
            match scope {
                PathScope::Subtree => format!("workspace:{}/**", joined),
                PathScope::Exact => format!("workspace:{}", joined),
            }
        };

        Self {
            namespace: ResourceNamespace::Workspace,
            path_components,
            scope,
            canonical_id,
        }
    }

    /// Create a repository resource key.
    pub fn repository(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            namespace: ResourceNamespace::Repository,
            path_components: vec![name.clone()],
            scope: PathScope::Exact,
            canonical_id: format!("repository:{}", name),
        }
    }

    /// Create a subsystem resource key.
    pub fn subsystem(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            namespace: ResourceNamespace::Subsystem,
            path_components: vec![name.clone()],
            scope: PathScope::Exact,
            canonical_id: format!("subsystem:{}", name),
        }
    }

    /// Create a capability resource key.
    pub fn capability(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            namespace: ResourceNamespace::Capability,
            path_components: vec![id.clone()],
            scope: PathScope::Exact,
            canonical_id: format!("capability:{}", id),
        }
    }

    /// Create a tool resource key.
    pub fn tool(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            namespace: ResourceNamespace::Tool,
            path_components: vec![name.clone()],
            scope: PathScope::Exact,
            canonical_id: format!("tool:{}", name),
        }
    }

    /// Create a custom resource key.
    pub fn custom(namespace: impl Into<String>, id: impl Into<String>) -> Self {
        let ns = namespace.into();
        let id = id.into();
        Self {
            namespace: ResourceNamespace::Custom(ns.clone()),
            path_components: vec![id.clone()],
            scope: PathScope::Exact,
            canonical_id: format!("{}:{}", ns, id),
        }
    }

    /// Determine whether this resource with `my_mode` conflicts with `other` with `other_mode`.
    ///
    /// Per D-01:
    /// - Shared / Shared is always compatible (never conflicts).
    /// - Exclusive / Shared, Shared / Exclusive, and Exclusive / Exclusive conflict if resources overlap.
    ///
    /// Per D-04:
    /// - Workspace paths use component-wise ancestry matching, preventing raw prefix collisions.
    pub fn conflicts_with(
        &self,
        my_mode: LockMode,
        other: &ResourceKey,
        other_mode: LockMode,
    ) -> bool {
        // Disjoint namespaces never conflict
        if self.namespace != other.namespace {
            return false;
        }

        // Shared / Shared is compatible
        if my_mode == LockMode::Shared && other_mode == LockMode::Shared {
            return false;
        }

        // Non-workspace resources conflict strictly on canonical ID identity
        if self.namespace != ResourceNamespace::Workspace {
            return self.canonical_id == other.canonical_id;
        }

        // Workspace resources: component-wise ancestry and scope matching
        if self.path_components == other.path_components {
            return true;
        }

        if self.scope == PathScope::Subtree
            && is_prefix_of(&self.path_components, &other.path_components)
        {
            return true;
        }

        if other.scope == PathScope::Subtree
            && is_prefix_of(&other.path_components, &self.path_components)
        {
            return true;
        }

        false
    }
}

fn is_prefix_of(prefix: &[String], full: &[String]) -> bool {
    prefix.len() <= full.len() && prefix == &full[..prefix.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subtree_conflicts_with_descendant_exact() {
        let tree = ResourceKey::workspace("src/planning", PathScope::Subtree);
        let file = ResourceKey::workspace("src/planning/candidate.rs", PathScope::Exact);

        assert!(tree.conflicts_with(LockMode::Exclusive, &file, LockMode::Exclusive));
        assert!(tree.conflicts_with(LockMode::Exclusive, &file, LockMode::Shared));
        assert!(file.conflicts_with(LockMode::Exclusive, &tree, LockMode::Shared));
        assert!(!tree.conflicts_with(LockMode::Shared, &file, LockMode::Shared));
    }

    #[test]
    fn test_component_segment_boundary_prevents_false_positive() {
        let tree = ResourceKey::workspace("src/planning", PathScope::Subtree);
        let sibling = ResourceKey::workspace("src/planning_old/foo.rs", PathScope::Exact);

        assert!(!tree.conflicts_with(LockMode::Exclusive, &sibling, LockMode::Exclusive));
        assert!(!sibling.conflicts_with(LockMode::Exclusive, &tree, LockMode::Exclusive));
    }

    #[test]
    fn test_shared_shared_identical_file_does_not_conflict() {
        let f1 = ResourceKey::workspace("src/dag/graph.rs", PathScope::Exact);
        let f2 = ResourceKey::workspace("src/dag/graph.rs", PathScope::Exact);

        assert!(!f1.conflicts_with(LockMode::Shared, &f2, LockMode::Shared));
    }

    #[test]
    fn test_exclusive_shared_identical_file_conflicts() {
        let f1 = ResourceKey::workspace("src/dag/graph.rs", PathScope::Exact);
        let f2 = ResourceKey::workspace("src/dag/graph.rs", PathScope::Exact);

        assert!(f1.conflicts_with(LockMode::Exclusive, &f2, LockMode::Shared));
        assert!(f1.conflicts_with(LockMode::Shared, &f2, LockMode::Exclusive));
        assert!(f1.conflicts_with(LockMode::Exclusive, &f2, LockMode::Exclusive));
    }

    #[test]
    fn test_exact_directory_does_not_conflict_with_descendant_file() {
        let dir = ResourceKey::workspace("src/planning", PathScope::Exact);
        let file = ResourceKey::workspace("src/planning/candidate.rs", PathScope::Exact);

        assert!(!dir.conflicts_with(LockMode::Exclusive, &file, LockMode::Exclusive));
        assert!(!file.conflicts_with(LockMode::Exclusive, &dir, LockMode::Exclusive));
    }

    #[test]
    fn test_disjoint_subtrees_do_not_conflict() {
        let tree1 = ResourceKey::workspace("src/dag", PathScope::Subtree);
        let tree2 = ResourceKey::workspace("src/planning", PathScope::Subtree);

        assert!(!tree1.conflicts_with(LockMode::Exclusive, &tree2, LockMode::Exclusive));
    }

    #[test]
    fn test_non_filesystem_namespaces() {
        let cap1 = ResourceKey::capability("cargo");
        let cap2 = ResourceKey::capability("cargo");
        let cap3 = ResourceKey::capability("docker");

        assert!(cap1.conflicts_with(LockMode::Exclusive, &cap2, LockMode::Exclusive));
        assert!(!cap1.conflicts_with(LockMode::Exclusive, &cap3, LockMode::Exclusive));

        let tool = ResourceKey::tool("cargo");
        // Different namespaces do not conflict even if name matches
        assert!(!cap1.conflicts_with(LockMode::Exclusive, &tool, LockMode::Exclusive));
    }
}
