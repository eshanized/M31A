use crate::error::M31AError;
use crate::events::EventBus;
use crate::events::envelope::EventEnvelope;
use crate::events::types::EventType;
use crate::ids::{MissionId, TaskGraphId, TaskId};
use std::sync::Arc;

/// Helper function to publish TaskGraphMaterialized event on EventBus (D-12).
pub async fn emit_graph_materialized(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    graph_id: TaskGraphId,
    revision: u32,
    task_count: usize,
    tasks: Vec<crate::events::types::TaskSummary>,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        "scheduler".to_string(),
        EventType::TaskGraphMaterialized {
            graph_id,
            mission_id,
            revision,
            task_count,
            tasks,
        },
    );
    bus.publish(envelope).await
}

/// Helper function to publish WaveTierComputed event on EventBus (D-12).
pub async fn emit_wave_tier(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    graph_id: TaskGraphId,
    wave_index: usize,
    task_ids: Vec<TaskId>,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        "scheduler".to_string(),
        EventType::WaveTierComputed {
            graph_id,
            wave_index,
            task_ids,
        },
    );
    bus.publish(envelope).await
}

/// Helper function to publish ResourceLeased event on EventBus (D-12).
pub async fn emit_resource_leased(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    lease_id: uuid::Uuid,
    task_id: TaskId,
    resource_key: String,
    lock_mode: String,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        "scheduler".to_string(),
        EventType::ResourceLeased {
            lease_id,
            task_id,
            resource_key,
            lock_mode,
        },
    );
    bus.publish(envelope).await
}

/// Helper function to publish ResourceReleased event on EventBus (D-12).
pub async fn emit_resource_released(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    lease_id: uuid::Uuid,
    task_id: TaskId,
    resource_key: String,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        "scheduler".to_string(),
        EventType::ResourceReleased {
            lease_id,
            task_id,
            resource_key,
        },
    );
    bus.publish(envelope).await
}

/// Helper function to publish TaskBlocked event on EventBus (D-12).
pub async fn emit_task_blocked(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    task_id: TaskId,
    reason: String,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        "scheduler".to_string(),
        EventType::TaskBlocked {
            task_id,
            mission_id,
            reason,
        },
    );
    bus.publish(envelope).await
}

/// Helper function to publish TaskUnblocked event on EventBus (D-12).
pub async fn emit_task_unblocked(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    task_id: TaskId,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        "scheduler".to_string(),
        EventType::TaskUnblocked {
            task_id,
            mission_id,
        },
    );
    bus.publish(envelope).await
}

/// Helper function to publish CriticalPathRecalculated event on EventBus (D-12).
pub async fn emit_critical_path_recalculated(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    graph_id: TaskGraphId,
    critical_tasks: Vec<TaskId>,
    projected_duration_secs: u64,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        "scheduler".to_string(),
        EventType::CriticalPathRecalculated {
            graph_id,
            critical_tasks,
            projected_duration_secs,
        },
    );
    bus.publish(envelope).await
}

/// Helper function to publish TaskStarted event on EventBus.
pub async fn emit_task_started(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    task_id: TaskId,
    agent_id: crate::ids::AgentId,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        "scheduler".to_string(),
        EventType::TaskStarted {
            task_id,
            mission_id,
            agent_id,
        },
    );
    bus.publish(envelope).await
}

/// Helper function to publish TaskCompleted event on EventBus.
pub async fn emit_task_completed(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    task_id: TaskId,
    result: String,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        "scheduler".to_string(),
        EventType::TaskCompleted {
            task_id,
            mission_id,
            result,
        },
    );
    bus.publish(envelope).await
}

/// Helper function to publish TaskFailed event on EventBus.
pub async fn emit_task_failed(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    task_id: TaskId,
    error: String,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        "scheduler".to_string(),
        EventType::TaskFailed {
            task_id,
            mission_id,
            error,
        },
    );
    bus.publish(envelope).await
}

/// Helper function to publish TaskCancelled event on EventBus.
pub async fn emit_task_cancelled(
    bus: &Arc<dyn EventBus>,
    sequence: u64,
    mission_id: MissionId,
    task_id: TaskId,
    reason: String,
) -> Result<(), M31AError> {
    let envelope = EventEnvelope::new(
        sequence,
        Some(mission_id),
        None,
        "scheduler".to_string(),
        EventType::TaskCancelled {
            task_id,
            mission_id,
            reason,
        },
    );
    bus.publish(envelope).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::bus::{BroadcastEventBus, EventFilter};
    use futures::StreamExt;

    #[tokio::test]
    async fn test_emit_scheduler_events() {
        let bus: Arc<dyn EventBus> = Arc::new(BroadcastEventBus::new(16));
        let mission_id = MissionId::new();
        let graph_id = TaskGraphId::new();
        let task_id = TaskId::new();

        let mut stream = bus.subscribe(EventFilter::all()).await;

        emit_graph_materialized(&bus, 1, mission_id, graph_id, 1, 3, vec![])
            .await
            .unwrap();
        emit_task_blocked(&bus, 2, mission_id, task_id, "prerequisite failed".into())
            .await
            .unwrap();

        let e1 = stream.next().await.unwrap().unwrap();
        assert_eq!(e1.event_type.name(), "TaskGraphMaterialized");

        let e2 = stream.next().await.unwrap().unwrap();
        assert_eq!(e2.event_type.name(), "TaskBlocked");
    }
}
