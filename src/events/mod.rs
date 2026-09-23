//! Events module - typed event contracts and sequencing (KRN-03)
//!
//! Per D-16, events/ owns typed event contracts and event sequencing.

pub mod bus;
pub mod envelope;
pub mod types;

pub use bus::{BroadcastEventBus, EventBus, EventFilter, EventReceiver};
pub use envelope::{EventCategory, EventEnvelope};
pub use types::EventType;
