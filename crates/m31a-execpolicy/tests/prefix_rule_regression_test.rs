//! Regression tests for P1 Persistent Approval Rule Escalation.

use m31a_execpolicy::{Decision, ExecPolicy, PrefixRule};

#[test]
fn test_prefix_rule_from_pattern_preserves_subcommands() {
    let mut policy = ExecPolicy::standard();

    // User approves "cargo test"
    let rule = PrefixRule::from_pattern(
        "cargo test",
        Decision::Allow,
        Some("User approved cargo test".to_string()),
    )
    .expect("Valid pattern");

    policy.add_prefix_rule(rule);

    // "cargo test" and "cargo test --all" should be allowed
    assert_eq!(policy.evaluate("cargo test"), Decision::Allow);
    assert_eq!(policy.evaluate("cargo test --all"), Decision::Allow);

    // "cargo publish" and "cargo clean" MUST NOT be allowed!
    assert_ne!(
        policy.evaluate("cargo publish"),
        Decision::Allow,
        "cargo publish must not be allowed by a cargo test rule!"
    );
    assert_ne!(
        policy.evaluate("cargo clean"),
        Decision::Allow,
        "cargo clean must not be allowed by a cargo test rule!"
    );
}

#[test]
fn test_deny_rule_cannot_be_overridden_by_allow_rule() {
    let mut policy = ExecPolicy::standard();

    // Deny rule for git push
    let deny_rule = PrefixRule::from_pattern(
        "git push",
        Decision::Deny,
        Some("Explicit deny git push".to_string()),
    )
    .expect("Valid deny rule");
    policy.add_prefix_rule(deny_rule);

    // Subsequent broad allow rule for git
    let allow_rule =
        PrefixRule::from_pattern("git", Decision::Allow, Some("Broad allow git".to_string()))
            .expect("Valid allow rule");
    policy.add_prefix_rule(allow_rule);

    // git push must remain Deny!
    assert_eq!(
        policy.evaluate("git push origin main"),
        Decision::Deny,
        "Deny rule must take precedence over broad allow rule!"
    );
}

#[test]
fn test_empty_and_malformed_patterns_rejected() {
    assert!(PrefixRule::from_pattern("", Decision::Allow, None).is_err());
    assert!(PrefixRule::from_pattern("   ", Decision::Allow, None).is_err());
    assert!(PrefixRule::from_pattern("echo \"unclosed quote", Decision::Allow, None).is_err());
}

#[test]
fn test_quoted_arguments_handled_correctly() {
    let rule = PrefixRule::from_pattern("python -c 'print(1)'", Decision::Allow, None)
        .expect("Valid quoted rule");

    assert_eq!(rule.program, "python");
    assert_eq!(rule.rest.len(), 2);
}
