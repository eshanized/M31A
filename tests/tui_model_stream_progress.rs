use m31a::interaction::events::InteractionEvent;
use m31a::model::types::{CostProvenance, TokenUsage, UsageSource};
use m31a::tui::conversation::TuiConversationItem;
use m31a::tui::model::TuiViewModel;

#[test]
fn live_model_progress_displays_elapsed_time_and_truthful_token_provenance() {
    let mut model = TuiViewModel::new();

    model.apply_interaction_event(&InteractionEvent::ModelStreamProgress {
        invocation_id: "invocation-test-1".to_string(),
        provider: "nvidia_nim".to_string(),
        model: "reasoning-model".to_string(),
        elapsed_ms: 13_000,
        reasoning_tokens_estimated: Some(234),
        usage: None,
    });

    let initial = model.thought_duration_label().expect("thinking label");
    assert!(initial.contains("Thought"));
    assert!(initial.contains("13.0s"));
    assert!(initial.contains("~234 reasoning tokens"));

    model.apply_interaction_event(&InteractionEvent::ModelStreamProgress {
        invocation_id: "invocation-test-1".to_string(),
        provider: "nvidia_nim".to_string(),
        model: "reasoning-model".to_string(),
        elapsed_ms: 13_000,
        reasoning_tokens_estimated: Some(234),
        usage: Some(TokenUsage::new(
            100,
            400,
            500,
            234,
            UsageSource::AuthoritativeProvider,
        )),
    });

    let provider_usage = model.thought_duration_label().expect("live usage label");
    assert!(provider_usage.contains("500 tokens (provider)"));
    assert!(provider_usage.contains("234 reasoning"));

    model.apply_interaction_event(&InteractionEvent::ModelUsageUpdated {
        invocation_id: Some("invocation-test-1".to_string()),
        prompt_tokens: 100,
        completion_tokens: 400,
        total_tokens: 500,
        cost_cents: None,
        cost_usd: None,
        cost_provenance: CostProvenance::Unknown,
    });

    assert!(
        model.conversation.iter().any(|item| matches!(
            item,
            TuiConversationItem::System { text, .. }
                if text.contains("Thought for 13s, 234 reasoning tokens")
        )),
        "completed invocation should leave a compact thought-duration summary"
    );
    assert!(model.live_model_invocation_id.is_none());
}

#[test]
fn missing_provider_usage_is_never_presented_as_authoritative() {
    let mut model = TuiViewModel::new();
    model.apply_interaction_event(&InteractionEvent::ModelStreamProgress {
        invocation_id: "invocation-test-2".to_string(),
        provider: "nvidia_nim".to_string(),
        model: "reasoning-model".to_string(),
        elapsed_ms: 1_000,
        reasoning_tokens_estimated: Some(9),
        usage: None,
    });

    let label = model.thought_duration_label().expect("thinking label");
    assert!(label.contains("~9 reasoning tokens"));
    assert!(!label.contains("9 reasoning tokens (provider)"));
}

#[test]
fn final_usage_without_stream_provenance_does_not_invent_a_token_source() {
    let mut model = TuiViewModel::new();
    model.apply_interaction_event(&InteractionEvent::ModelStreamProgress {
        invocation_id: "invocation-test-3".to_string(),
        provider: "nvidia_nim".to_string(),
        model: "reasoning-model".to_string(),
        elapsed_ms: 3_000,
        reasoning_tokens_estimated: Some(75),
        usage: None,
    });

    model.apply_interaction_event(&InteractionEvent::ModelUsageUpdated {
        invocation_id: Some("invocation-test-3".to_string()),
        prompt_tokens: 400,
        completion_tokens: 100,
        total_tokens: 500,
        cost_cents: None,
        cost_usd: None,
        cost_provenance: CostProvenance::Unknown,
    });

    assert!(
        model.conversation.iter().any(|item| matches!(
            item,
            TuiConversationItem::System { text, .. }
                if text.contains("~75 reasoning tokens (estimated)")
                    && !text.contains("500 tokens (provider)")
        ))
    );
}

#[test]
fn late_progress_does_not_reopen_a_completed_thought_indicator() {
    let mut model = TuiViewModel::new();
    model.apply_interaction_event(&InteractionEvent::ModelStreamProgress {
        invocation_id: "invocation-test-late".to_string(),
        provider: "nvidia_nim".to_string(),
        model: "reasoning-model".to_string(),
        elapsed_ms: 2_000,
        reasoning_tokens_estimated: Some(18),
        usage: None,
    });
    model.apply_interaction_event(&InteractionEvent::ModelUsageUpdated {
        invocation_id: Some("invocation-test-late".to_string()),
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: 0,
        cost_cents: None,
        cost_usd: None,
        cost_provenance: CostProvenance::Unknown,
    });

    model.apply_interaction_event(&InteractionEvent::ModelStreamProgress {
        invocation_id: "invocation-test-late".to_string(),
        provider: "nvidia_nim".to_string(),
        model: "reasoning-model".to_string(),
        elapsed_ms: 2_500,
        reasoning_tokens_estimated: Some(24),
        usage: None,
    });

    assert!(model.live_model_invocation_id.is_none());
    assert!(model.thought_duration_label().is_none());
}

#[test]
fn failed_invocation_closes_thinking_and_ignores_late_progress() {
    let mut model = TuiViewModel::new();
    model.apply_interaction_event(&InteractionEvent::ModelStreamProgress {
        invocation_id: "invocation-test-failed".to_string(),
        provider: "nvidia_nim".to_string(),
        model: "reasoning-model".to_string(),
        elapsed_ms: 1_500,
        reasoning_tokens_estimated: Some(12),
        usage: None,
    });

    model.apply_interaction_event(&InteractionEvent::ModelStreamFinished {
        invocation_id: "invocation-test-failed".to_string(),
        failed: true,
    });

    assert!(model.live_model_invocation_id.is_none());
    assert!(model.thought_duration_label().is_none());

    model.apply_interaction_event(&InteractionEvent::ModelStreamProgress {
        invocation_id: "invocation-test-failed".to_string(),
        provider: "nvidia_nim".to_string(),
        model: "reasoning-model".to_string(),
        elapsed_ms: 2_000,
        reasoning_tokens_estimated: Some(16),
        usage: None,
    });

    assert!(model.live_model_invocation_id.is_none());
    assert!(model.thought_duration_label().is_none());
}
