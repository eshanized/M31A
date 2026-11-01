//! Dedicated integration tests for `m31a-fuzzy` (src/matcher.rs).
//!
//! Validates:
//! - Subsequence matching, empty queries, and length boundaries
//! - Boundary bonuses (path delimiters `/`, separators `_`, `-`, `.`, and CamelCase transitions)
//! - Consecutive match rewards vs distance penalties
//! - Match indices alignment with target character positions
//! - Unicode and ASCII zero-heap fast-path fidelity

use m31a_fuzzy::matcher::{fuzzy_match, fuzzy_match_indices};

#[test]
fn test_boundary_and_empty_conditions() {
    // Empty query matches everything with score 0 and empty indices
    assert_eq!(fuzzy_match("", "anything.rs"), Some(0));
    assert_eq!(fuzzy_match_indices("", "anything.rs"), Some((0, vec![])));

    // Query longer than target returns None
    assert_eq!(fuzzy_match("very_long_query_string", "short.rs"), None);

    // Non-subsequence returns None
    assert_eq!(fuzzy_match("abc", "def.rs"), None);
    assert_eq!(fuzzy_match("z", "crates/m31a-core/src/lib.rs"), None);
}

#[test]
fn test_exact_match_score_boost() {
    let exact_score = fuzzy_match("main.rs", "main.rs").unwrap();
    let partial_score = fuzzy_match("main.rs", "crates/main.rs.bak").unwrap();
    assert!(
        exact_score > partial_score + 100,
        "Exact match should gain 150-point bonus: exact={}, partial={}",
        exact_score,
        partial_score
    );
}

#[test]
fn test_boundary_preference_heuristics() {
    // Path delimiter bonus: 'lib' matching 'src/lib.rs' should score higher than 'calibration.rs'
    let score_delim = fuzzy_match("lib", "src/lib.rs").unwrap();
    let score_inside = fuzzy_match("lib", "calibration.rs").unwrap();
    assert!(
        score_delim > score_inside,
        "Path delimiter boundary must score higher: {} vs {}",
        score_delim,
        score_inside
    );

    // Word separator bonus ('_')
    let score_snake = fuzzy_match("tc", "turn_context.rs").unwrap();
    let score_scattered = fuzzy_match("tc", "statistic.rs").unwrap();
    assert!(
        score_snake > score_scattered,
        "Snake case boundary must score higher: {} vs {}",
        score_snake,
        score_scattered
    );

    // CamelCase boundary bonus
    let score_camel = fuzzy_match("cb", "ContextBudget.rs").unwrap();
    let score_random = fuzzy_match("cb", "incubator.rs").unwrap();
    assert!(
        score_camel > score_random,
        "CamelCase boundary must score higher: {} vs {}",
        score_camel,
        score_random
    );
}

#[test]
fn test_match_indices_accuracy() {
    let target = "crates/m31a-tools/src/execute.rs";
    let (score, indices) = fuzzy_match_indices("exec", target).expect("match expected");
    assert!(score > 0);
    assert_eq!(indices.len(), 4);

    let target_chars: Vec<char> = target.chars().collect();
    let reconstructed: String = indices.iter().map(|&idx| target_chars[idx]).collect();
    assert_eq!(reconstructed.to_lowercase(), "exec");
}

#[test]
fn test_consecutive_vs_dispersed_matches() {
    let target = "workspace_change_batch.rs";
    let score_consec = fuzzy_match("change", target).unwrap();
    let score_dispersed = fuzzy_match("cgbh", target).unwrap();
    assert!(
        score_consec > score_dispersed,
        "Consecutive characters must gain consecutive bonuses: {} vs {}",
        score_consec,
        score_dispersed
    );
}

#[test]
fn test_unicode_matching_support() {
    // Non-ASCII characters (e.g. Greek, CJK, accents)
    let target = "src/αβγ_math/λ_calculus.rs";
    let matched = fuzzy_match_indices("αβ", target);
    assert!(matched.is_some());
    let (_, indices) = matched.unwrap();
    assert_eq!(indices.len(), 2);
}
