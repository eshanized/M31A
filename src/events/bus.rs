//! EventBus trait and BroadcastEventBus implementation (D-10, D-11, D-12, EVT-01, EVT-03).

use crate::error::M31AError;
use crate::events::envelope::{EventCategory, EventEnvelope};
use crate::ids::{MissionId, SessionId};
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use tokio::sync::broadcast;

/// Default bounded channel capacity for in-process broadcast fan-out (D-12).
pub const DEFAULT_EVENT_BUS_CAPACITY: usize = 2048;

/// Filter for event subscription matching criteria (EVT-03).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EventFilter {
    pub mission_id: Option<MissionId>,
    pub session_id: Option<SessionId>,
    pub event_types: Vec<String>,
    pub categories: Vec<EventCategory>,
    pub from_sequence: Option<u64>,
    pub to_sequence: Option<u64>,
}

impl EventFilter {
    /// Create a filter that matches all events.
    pub fn all() -> Self {
        Self::default()
    }

    /// Builder method to filter by mission ID.
    pub fn with_mission(mut self, mission_id: MissionId) -> Self {
        self.mission_id = Some(mission_id);
        self
    }

    /// Builder method to filter by session ID.
    pub fn with_session(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Builder method to filter by event type name.
    pub fn with_type(mut self, event_type: impl Into<String>) -> Self {
        self.event_types.push(event_type.into());
        self
    }

    /// Builder method to filter by event category.
    pub fn with_category(mut self, category: EventCategory) -> Self {
        self.categories.push(category);
        self
    }

    /// Builder method to filter events starting from sequence (inclusive).
    pub fn from_seq(mut self, sequence: u64) -> Self {
        self.from_sequence = Some(sequence);
        self
    }

    /// Builder method to filter events up to sequence (inclusive).
    pub fn to_seq(mut self, sequence: u64) -> Self {
        self.to_sequence = Some(sequence);
        self
    }

    /// Evaluates if an EventEnvelope matches all non-empty filter criteria.
    pub fn matches(&self, envelope: &EventEnvelope) -> bool {
        if self.mission_id.is_some() && envelope.mission_id != self.mission_id {
            return false;
        }

        if self.session_id.is_some() && envelope.session_id != self.session_id {
            return false;
        }

        if !self.event_types.is_empty() {
            let event_name = envelope.event_type.name();
            if !self.event_types.iter().any(|t| t == event_name) {
                return false;
            }
        }

        if !self.categories.is_empty() && !self.categories.contains(&envelope.category) {
            return false;
        }

        if self
            .from_sequence
            .is_some_and(|from| envelope.sequence < from)
        {
            return false;
        }

        if self.to_sequence.is_some_and(|to| envelope.sequence > to) {
            return false;
        }

        true
    }
}

/// Event receiver type alias - a stream of event envelopes.
pub type EventReceiver = Pin<Box<dyn Stream<Item = Result<EventEnvelope, M31AError>> + Send>>;

/// EventBus trait for publishing and subscribing to runtime events.
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publish an event envelope to the bus.
    async fn publish(&self, envelope: EventEnvelope) -> Result<(), M31AError>;

    /// Subscribe to events matching the filter.
    async fn subscribe(&self, filter: EventFilter) -> EventReceiver;
}

/// Concrete in-memory event bus backed by bounded `tokio::sync::broadcast` (D-10, D-12).
pub struct BroadcastEventBus {
    sender: broadcast::Sender<EventEnvelope>,
    capacity: usize,
}

impl BroadcastEventBus {
    /// Create a new BroadcastEventBus with the given buffer capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender, capacity }
    }

    /// Return the configured channel capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Return the count of currently active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for BroadcastEventBus {
    fn default() -> Self {
        Self::new(DEFAULT_EVENT_BUS_CAPACITY)
    }
}

#[async_trait]
impl EventBus for BroadcastEventBus {
    async fn publish(&self, envelope: EventEnvelope) -> Result<(), M31AError> {
        // Per D-10/D-11: Broadcast is ephemeral. If there are no receivers,
        // send() returns an error which we gracefully absorb as Ok(()).
        let _ = self.sender.send(envelope);
        Ok(())
    }

    async fn subscribe(&self, filter: EventFilter) -> EventReceiver {
        let rx = self.sender.subscribe();
        let stream = futures::stream::unfold((rx, filter), |(mut rx, filter)| async move {
            loop {
                match rx.recv().await {
                    Ok(envelope) => {
                        if filter.matches(&envelope) {
                            return Some((Ok(envelope), (rx, filter)));
                        }
                        // Continue loop if filter did not match
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        // Per D-12: Surface lagged notification so subscriber can catch up from SQLite
                        return Some((Err(M31AError::EventLagged { skipped }), (rx, filter)));
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        return None;
                    }
                }
            }
        });

        Box::pin(stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::EventType;
    use crate::ids::MissionId;
    use futures::StreamExt;

    #[test]
    fn test_event_filter_builder_and_matching() {
        let mission1 = MissionId::new();
        let mission2 = MissionId::new();

        let filter = EventFilter::all()
            .with_mission(mission1)
            .with_category(EventCategory::Durable)
            .from_seq(5)
            .to_seq(10);

        let matching_envelope = EventEnvelope::new(
            7,
            Some(mission1),
            None,
            "test".to_string(),
            EventType::MissionStarted {
                mission_id: mission1,
                objective: "obj".to_string(),
            },
        );

        let wrong_mission = EventEnvelope::new(
            7,
            Some(mission2),
            None,
            "test".to_string(),
            EventType::MissionStarted {
                mission_id: mission2,
                objective: "obj".to_string(),
            },
        );

        let out_of_range = EventEnvelope::new(
            12,
            Some(mission1),
            None,
            "test".to_string(),
            EventType::MissionStarted {
                mission_id: mission1,
                objective: "obj".to_string(),
            },
        );

        let wrong_category = EventEnvelope::new(
            8,
            Some(mission1),
            None,
            "test".to_string(),
            EventType::MissionStarted {
                mission_id: mission1,
                objective: "obj".to_string(),
            },
        )
        .with_category(EventCategory::Ephemeral);

        assert!(filter.matches(&matching_envelope));
        assert!(!filter.matches(&wrong_mission));
        assert!(!filter.matches(&out_of_range));
        assert!(!filter.matches(&wrong_category));
    }

    #[tokio::test]
    async fn test_broadcast_event_bus_publish_no_subscribers() {
        let bus = BroadcastEventBus::new(16);
        let envelope = EventEnvelope::new(
            1,
            None,
            None,
            "test".to_string(),
            EventType::MissionCompleted {
                mission_id: MissionId::new(),
            },
        );
        // Publishing with no receivers succeeds without error
        assert!(bus.publish(envelope).await.is_ok());
    }

    #[tokio::test]
    async fn test_broadcast_event_bus_delivery_and_filtering() {
        let bus = BroadcastEventBus::new(16);
        let mission_id = MissionId::new();

        let filter = EventFilter::all().with_mission(mission_id);
        let mut stream = bus.subscribe(filter).await;

        let matching_event = EventEnvelope::new(
            1,
            Some(mission_id),
            None,
            "actor".to_string(),
            EventType::MissionStarted {
                mission_id,
                objective: "test".to_string(),
            },
        );

        let other_event = EventEnvelope::new(
            2,
            Some(MissionId::new()),
            None,
            "actor".to_string(),
            EventType::MissionCompleted {
                mission_id: MissionId::new(),
            },
        );

        bus.publish(other_event).await.unwrap();
        bus.publish(matching_event.clone()).await.unwrap();

        let received = stream.next().await.unwrap().unwrap();
        assert_eq!(received.id, matching_event.id);
        assert_eq!(received.mission_id, Some(mission_id));
    }

    #[tokio::test]
    async fn test_broadcast_event_bus_lag_detection() {
        // Small capacity channel to force lagging
        let bus = BroadcastEventBus::new(2);
        let mut stream = bus.subscribe(EventFilter::all()).await;

        for i in 1..=10 {
            let env = EventEnvelope::new(
                i,
                None,
                None,
                "actor".to_string(),
                EventType::MissionCompleted {
                    mission_id: MissionId::new(),
                },
            );
            bus.publish(env).await.unwrap();
        }

        // The receiver should receive an EventLagged error
        let result = stream.next().await.unwrap();
        assert!(matches!(result, Err(M31AError::EventLagged { .. })));
    }
}
