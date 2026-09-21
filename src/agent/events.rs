//! Agent lifecycle event publishers (D-15).
//!
//! Emits first-class typed domain events (AgentSpawned, AgentStarted, AgentStepCompleted,
//! AgentHandoffRecorded, AgentCompleted, AgentFailed, AgentCancelled) to EventBus.
//! High-frequency telemetry (heartbeats, token chunks) is kept out of durable events.

use std::sync::Arc;

use crate::error::M31AError;
use crate::events::bus::EventBus;
use crate::events::envelope::EventEnvelope;
use crate::events::types::EventType;
use crate::ids::{AgentId, HandoffId, MissionId, TaskId};

/// Publish an AgentSpawned event on the EventBus.
pub async fn emit_agent_spawned(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    agent_id: AgentId,
    role: &str,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        format!("agent:{}", agent_id),
        EventType::AgentSpawned {
            agent_id,
            mission_id,
            role: role.to_string(),
        },
    );
    bus.publish(envelope).await
}

/// Publish an AgentStarted event on the EventBus.
pub async fn emit_agent_started(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    agent_id: AgentId,
    task_id: TaskId,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        format!("agent:{}", agent_id),
        EventType::AgentStarted {
            agent_id,
            mission_id,
            task_id,
        },
    );
    bus.publish(envelope).await
}

/// Publish an AgentStepCompleted event on the EventBus.
pub async fn emit_agent_step_completed(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    agent_id: AgentId,
    task_id: TaskId,
    step_number: u32,
    steps_remaining: u32,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        format!("agent:{}", agent_id),
        EventType::AgentStepCompleted {
            agent_id,
            task_id,
            step_number,
            steps_remaining,
        },
    );
    bus.publish(envelope).await
}

/// Publish an AgentHandoffRecorded event on the EventBus.
pub async fn emit_agent_handoff_recorded(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    handoff_id: HandoffId,
    source_agent_id: AgentId,
    target_role: &str,
    reason: &str,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        format!("agent:{}", source_agent_id),
        EventType::AgentHandoffRecorded {
            handoff_id,
            mission_id,
            source_agent_id,
            target_role: target_role.to_string(),
            reason: reason.to_string(),
        },
    );
    bus.publish(envelope).await
}

/// Publish an AgentCompleted event on the EventBus.
pub async fn emit_agent_completed(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    agent_id: AgentId,
    summary: &str,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        format!("agent:{}", agent_id),
        EventType::AgentCompleted {
            agent_id,
            mission_id,
            summary: summary.to_string(),
        },
    );
    bus.publish(envelope).await
}

/// Publish an AgentFailed event on the EventBus.
pub async fn emit_agent_failed(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    agent_id: AgentId,
    error: &str,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        format!("agent:{}", agent_id),
        EventType::AgentFailed {
            agent_id,
            mission_id,
            error: error.to_string(),
        },
    );
    bus.publish(envelope).await
}

/// Publish an AgentCancelled event on the EventBus.
pub async fn emit_agent_cancelled(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    agent_id: AgentId,
    reason: &str,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        format!("agent:{}", agent_id),
        EventType::AgentCancelled {
            agent_id,
            mission_id,
            reason: reason.to_string(),
        },
    );
    bus.publish(envelope).await
}
