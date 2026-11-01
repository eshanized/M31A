//! Accessibility tree parser and `@e1`, `@e2` element reference allocator.
//!
//! Translates raw CDP Accessibility / DOM trees into compact, token-efficient,
//! human-readable hierarchies annotated with stable `@e<id>` references for agent interaction.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An element reference identifier (e.g. `@e1`, `@e2`).
pub type ElementRef = String;

/// A parsed accessibility node in the active page hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityNode {
    pub ref_id: Option<ElementRef>,
    pub role: String,
    pub name: String,
    pub value: Option<String>,
    pub backend_node_id: Option<i64>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub children: Vec<AccessibilityNode>,
}

/// A snapshot of the browser page ready for agent reasoning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSnapshot {
    pub url: String,
    pub title: String,
    pub tree_text: String,
    pub ref_map: HashMap<ElementRef, (f64, f64, Option<i64>)>, // ref -> (center_x, center_y, backend_node_id)
}

/// Allocates sequential `@e1`, `@e2`, ... references and formats the accessibility tree.
pub struct AccessibilityTreeBuilder {
    next_ref_id: usize,
    ref_map: HashMap<ElementRef, (f64, f64, Option<i64>)>,
}

impl Default for AccessibilityTreeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessibilityTreeBuilder {
    pub fn new() -> Self {
        Self {
            next_ref_id: 1,
            ref_map: HashMap::new(),
        }
    }

    /// Determines if an accessibility role is interactive or significant enough
    /// to warrant allocating an agent-accessible `@e<n>` reference.
    fn is_interactive_role(role: &str) -> bool {
        matches!(
            role.to_ascii_lowercase().as_str(),
            "button"
                | "link"
                | "textbox"
                | "checkbox"
                | "radio"
                | "combobox"
                | "menuitem"
                | "tab"
                | "searchbox"
                | "slider"
                | "switch"
                | "option"
        )
    }

    /// Recursively indexes accessibility nodes, assigning `@e<n>` refs to interactive elements.
    pub fn index_node(&mut self, mut node: AccessibilityNode) -> AccessibilityNode {
        if Self::is_interactive_role(&node.role)
            || (!node.name.is_empty() && node.ref_id.is_none() && node.width > 0.0)
        {
            let ref_str = format!("@e{}", self.next_ref_id);
            self.next_ref_id += 1;

            let center_x = node.x + (node.width / 2.0);
            let center_y = node.y + (node.height / 2.0);
            self.ref_map
                .insert(ref_str.clone(), (center_x, center_y, node.backend_node_id));

            node.ref_id = Some(ref_str);
        }

        let mut indexed_children = Vec::with_capacity(node.children.len());
        for child in node.children {
            indexed_children.push(self.index_node(child));
        }
        node.children = indexed_children;

        node
    }

    /// Formats an indexed accessibility node and its children into indented text representation.
    pub fn format_tree(&self, node: &AccessibilityNode, indent: usize) -> String {
        let mut out = String::new();
        let pad = "  ".repeat(indent);

        let ref_part = if let Some(ref r) = node.ref_id {
            format!("{} ", r)
        } else {
            String::new()
        };

        let val_part = if let Some(ref v) = node.value {
            format!(" [value: \"{}\"]", v)
        } else {
            String::new()
        };

        if !node.name.is_empty() || !node.role.is_empty() {
            out.push_str(&format!(
                "{}{}{} \"{}\"{}\n",
                pad, ref_part, node.role, node.name, val_part
            ));
        }

        for child in &node.children {
            out.push_str(&self.format_tree(child, indent + 1));
        }

        out
    }

    /// Consumes the builder, producing the full page snapshot.
    pub fn build(mut self, url: &str, title: &str, root: AccessibilityNode) -> PageSnapshot {
        let indexed_root = self.index_node(root);
        let mut tree_text = format!("[Page: \"{}\", URL: \"{}\"]\n", title, url);
        tree_text.push_str(&self.format_tree(&indexed_root, 1));

        PageSnapshot {
            url: url.to_string(),
            title: title.to_string(),
            tree_text,
            ref_map: self.ref_map,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accessibility_ref_allocation() {
        let root = AccessibilityNode {
            ref_id: None,
            role: "RootWebArea".to_string(),
            name: "GitHub Login".to_string(),
            value: None,
            backend_node_id: Some(1),
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 800.0,
            children: vec![
                AccessibilityNode {
                    ref_id: None,
                    role: "heading".to_string(),
                    name: "Sign in".to_string(),
                    value: None,
                    backend_node_id: Some(2),
                    x: 100.0,
                    y: 50.0,
                    width: 200.0,
                    height: 40.0,
                    children: Vec::new(),
                },
                AccessibilityNode {
                    ref_id: None,
                    role: "textbox".to_string(),
                    name: "Username".to_string(),
                    value: Some("".to_string()),
                    backend_node_id: Some(3),
                    x: 100.0,
                    y: 100.0,
                    width: 300.0,
                    height: 35.0,
                    children: Vec::new(),
                },
                AccessibilityNode {
                    ref_id: None,
                    role: "button".to_string(),
                    name: "Sign in".to_string(),
                    value: None,
                    backend_node_id: Some(4),
                    x: 100.0,
                    y: 150.0,
                    width: 120.0,
                    height: 35.0,
                    children: Vec::new(),
                },
            ],
        };

        let builder = AccessibilityTreeBuilder::new();
        let snapshot = builder.build("https://github.com/login", "GitHub Login", root);

        assert!(snapshot.tree_text.contains("@e"));
        assert!(snapshot.tree_text.contains("button \"Sign in\""));
        assert!(snapshot.ref_map.contains_key("@e1"));
        assert!(snapshot.ref_map.contains_key("@e2"));
    }
}
