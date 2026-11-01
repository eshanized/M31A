//! Dedicated integration tests for `m31a-browser` (src/dom_tree.rs).
//!
//! Validates:
//! - Recursive accessibility node tree traversal and indexing
//! - Interactive element reference (`@e1`, `@e2`) assignment and center coordinate calculation
//! - Textual hierarchical tree formatting with role, name, and input values
//! - `PageSnapshot` assembly and reference map lookup accuracy

use m31a_browser::dom_tree::{AccessibilityNode, AccessibilityTreeBuilder};

#[test]
fn test_accessibility_tree_indexing_and_coordinates() {
    let root = AccessibilityNode {
        ref_id: None,
        role: "RootWebArea".to_string(),
        name: "Test Portal".to_string(),
        value: None,
        backend_node_id: Some(10),
        x: 0.0,
        y: 0.0,
        width: 1000.0,
        height: 600.0,
        children: vec![
            AccessibilityNode {
                ref_id: None,
                role: "button".to_string(),
                name: "Submit Order".to_string(),
                value: None,
                backend_node_id: Some(11),
                x: 100.0,
                y: 200.0,
                width: 80.0,
                height: 40.0,
                children: Vec::new(),
            },
            AccessibilityNode {
                ref_id: None,
                role: "textbox".to_string(),
                name: "Email".to_string(),
                value: Some("alice@example.com".to_string()),
                backend_node_id: Some(12),
                x: 100.0,
                y: 100.0,
                width: 200.0,
                height: 30.0,
                children: Vec::new(),
            },
        ],
    };

    let builder = AccessibilityTreeBuilder::new();
    let snapshot = builder.build("https://shop.example.com", "Test Portal", root);

    // Verify snapshot fields
    assert_eq!(snapshot.url, "https://shop.example.com");
    assert_eq!(snapshot.title, "Test Portal");
    assert!(
        snapshot
            .tree_text
            .contains("[Page: \"Test Portal\", URL: \"https://shop.example.com\"]")
    );

    // Verify interactive elements assigned refs
    assert!(snapshot.ref_map.contains_key("@e1") || snapshot.ref_map.contains_key("@e2"));

    // Check calculated coordinates for the button
    // Either @e1 or @e2 must correspond to button (center: x = 100 + 40 = 140, y = 200 + 20 = 220)
    let has_button_coords = snapshot
        .ref_map
        .values()
        .any(|&(cx, cy, node_id)| cx == 140.0 && cy == 220.0 && node_id == Some(11));
    assert!(
        has_button_coords,
        "Button center coordinates must be calculated as (140, 220)"
    );

    // Check email textbox value is rendered in formatted tree
    assert!(
        snapshot
            .tree_text
            .contains("textbox \"Email\" [value: \"alice@example.com\"]")
    );
    assert!(snapshot.tree_text.contains("button \"Submit Order\""));
}

#[test]
fn test_non_interactive_elements_handling() {
    let root = AccessibilityNode {
        ref_id: None,
        role: "generic".to_string(),
        name: "".to_string(),
        value: None,
        backend_node_id: None,
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
        children: Vec::new(),
    };

    let builder = AccessibilityTreeBuilder::new();
    let snapshot = builder.build("about:blank", "Blank", root);

    assert!(
        snapshot.ref_map.is_empty(),
        "Empty non-interactive node must not receive an @e ref"
    );
}
