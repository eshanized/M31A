//! Local terminal session capability provider (CTL-02, D-01, D-03).

use crate::capability::error::CapabilityError;
use crate::capability::traits::terminal::TerminalService;
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Native local terminal provider with in-memory session stream buffers.
pub struct LocalTerminalProvider {
    sessions: Arc<RwLock<HashMap<String, VecDeque<u8>>>>,
}

impl Default for LocalTerminalProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalTerminalProvider {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl TerminalService for LocalTerminalProvider {
    async fn write_input(&self, session_id: &str, input: &[u8]) -> Result<(), CapabilityError> {
        let mut sessions = self.sessions.write().unwrap();
        let buffer = sessions.entry(session_id.to_string()).or_default();
        buffer.extend(input);
        Ok(())
    }

    async fn read_stream(
        &self,
        session_id: &str,
        timeout_ms: u64,
    ) -> Result<Vec<u8>, CapabilityError> {
        let start = tokio::time::Instant::now();
        let timeout = Duration::from_millis(timeout_ms);

        loop {
            {
                let mut sessions = self.sessions.write().unwrap();
                if let Some(buffer) = sessions.get_mut(session_id)
                    && !buffer.is_empty()
                {
                    return Ok(buffer.drain(..).collect());
                }
            }

            if start.elapsed() >= timeout {
                return Ok(Vec::new());
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}
