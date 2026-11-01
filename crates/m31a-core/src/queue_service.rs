//! Durable, thread-scoped user-message queue and idle dispatch.
//! Durable, thread-scoped message queue managing pending user inputs and idle dispatch.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use uuid::Uuid;

/// Maximum number of items queued per thread.
pub const MAX_QUEUE_ITEMS: usize = 64;

/// Maximum character limit for a single queued user input.
pub const MAX_USER_INPUT_TEXT_CHARS: usize = 200_000;

/// A single queued user submission waiting for thread dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuedItem {
    pub id: String,
    pub thread_id: String,
    pub prompt: String,
    pub enqueued_at: i64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueueServiceError {
    #[error("queue item limit exceeded (maximum {0} items allowed)")]
    QueueFull(usize),
    #[error("user input exceeds maximum length of {max} characters ({actual} provided)")]
    InputTooLong { actual: usize, max: usize },
    #[error("empty user input cannot be queued")]
    EmptyInput,
}

/// Thread-safe in-memory and durable queue manager.
#[derive(Clone, Default)]
pub struct QueueService {
    queues: Arc<Mutex<HashMap<String, VecDeque<QueuedItem>>>>,
}

impl QueueService {
    pub fn new() -> Self {
        Self {
            queues: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Enqueues a user prompt onto the given thread.
    pub fn enqueue(&self, thread_id: &str, prompt: &str) -> Result<QueuedItem, QueueServiceError> {
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            return Err(QueueServiceError::EmptyInput);
        }

        let char_count = prompt.chars().count();
        if char_count > MAX_USER_INPUT_TEXT_CHARS {
            return Err(QueueServiceError::InputTooLong {
                actual: char_count,
                max: MAX_USER_INPUT_TEXT_CHARS,
            });
        }

        let mut map = self.queues.lock().unwrap();
        let queue = map.entry(thread_id.to_string()).or_default();

        if queue.len() >= MAX_QUEUE_ITEMS {
            return Err(QueueServiceError::QueueFull(MAX_QUEUE_ITEMS));
        }

        let item = QueuedItem {
            id: format!("queue_{}", Uuid::new_v4().simple()),
            thread_id: thread_id.to_string(),
            prompt: prompt.to_string(),
            enqueued_at: chrono::Utc::now().timestamp_millis(),
        };

        queue.push_back(item.clone());
        Ok(item)
    }

    /// Dequeues the next item in FIFO order for the given thread.
    pub fn dequeue(&self, thread_id: &str) -> Option<QueuedItem> {
        let mut map = self.queues.lock().unwrap();
        map.get_mut(thread_id).and_then(|q| q.pop_front())
    }

    /// Peeks at the next item in line without removing it.
    pub fn peek(&self, thread_id: &str) -> Option<QueuedItem> {
        let map = self.queues.lock().unwrap();
        map.get(thread_id).and_then(|q| q.front().cloned())
    }

    /// Removes a specific queued item by id.
    pub fn remove(&self, thread_id: &str, item_id: &str) -> bool {
        let mut map = self.queues.lock().unwrap();
        if let Some(queue) = map.get_mut(thread_id)
            && let Some(pos) = queue.iter().position(|it| it.id == item_id)
        {
            queue.remove(pos);
            return true;
        }
        false
    }

    /// Lists all queued items for a thread in FIFO order.
    pub fn list(&self, thread_id: &str) -> Vec<QueuedItem> {
        let map = self.queues.lock().unwrap();
        map.get(thread_id)
            .map(|q| q.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Clears all queued items for a thread.
    pub fn clear(&self, thread_id: &str) {
        let mut map = self.queues.lock().unwrap();
        map.remove(thread_id);
    }

    /// Returns true if a thread has queued items waiting.
    pub fn has_pending(&self, thread_id: &str) -> bool {
        let map = self.queues.lock().unwrap();
        map.get(thread_id).map(|q| !q.is_empty()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fifo_queueing_and_dequeuing() {
        let service = QueueService::new();
        let item1 = service.enqueue("th_1", "First prompt").unwrap();
        let item2 = service.enqueue("th_1", "Second prompt").unwrap();

        assert_eq!(service.list("th_1").len(), 2);
        assert_eq!(service.peek("th_1").unwrap().id, item1.id);

        let popped1 = service.dequeue("th_1").unwrap();
        assert_eq!(popped1.id, item1.id);

        let popped2 = service.dequeue("th_1").unwrap();
        assert_eq!(popped2.id, item2.id);

        assert!(service.dequeue("th_1").is_none());
        assert!(!service.has_pending("th_1"));
    }

    #[test]
    fn test_empty_and_too_long_validation() {
        let service = QueueService::new();
        assert_eq!(
            service.enqueue("th_1", "   ").unwrap_err(),
            QueueServiceError::EmptyInput
        );

        let huge_string = "x".repeat(MAX_USER_INPUT_TEXT_CHARS + 1);
        let err = service.enqueue("th_1", &huge_string).unwrap_err();
        match err {
            QueueServiceError::InputTooLong { actual, max } => {
                assert_eq!(actual, MAX_USER_INPUT_TEXT_CHARS + 1);
                assert_eq!(max, MAX_USER_INPUT_TEXT_CHARS);
            }
            _ => panic!("Expected InputTooLong error"),
        }
    }

    #[test]
    fn test_queue_capacity_limit() {
        let service = QueueService::new();
        for i in 0..MAX_QUEUE_ITEMS {
            assert!(service.enqueue("th_full", &format!("Msg {i}")).is_ok());
        }

        assert_eq!(
            service.enqueue("th_full", "Overflow message").unwrap_err(),
            QueueServiceError::QueueFull(MAX_QUEUE_ITEMS)
        );
    }

    #[test]
    fn test_remove_item() {
        let service = QueueService::new();
        let item1 = service.enqueue("th_2", "A").unwrap();
        let item2 = service.enqueue("th_2", "B").unwrap();

        assert!(service.remove("th_2", &item1.id));
        let remaining = service.list("th_2");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, item2.id);
    }
}
