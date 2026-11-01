//! Tracks pending approval blocking intervals and durations across turns.
//! Measures approval wait times, user confirmation blocking intervals, and turn pause metrics.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Default)]
struct Interval {
    active: usize,
    since: Option<Instant>,
    elapsed: Duration,
}

impl Interval {
    fn start_turn(&mut self, now: Instant) {
        self.elapsed = Duration::ZERO;
        self.since = (self.active > 0).then_some(now);
    }

    fn finish_turn(&mut self, now: Instant) -> Duration {
        if let Some(since) = self.since.take() {
            self.elapsed += now.saturating_duration_since(since);
        }
        std::mem::take(&mut self.elapsed)
    }

    fn enter(&mut self, now: Instant) {
        if self.active == 0 {
            self.since = Some(now);
        }
        self.active += 1;
    }

    fn leave(&mut self, now: Instant) {
        self.active = self.active.saturating_sub(1);
        if self.active == 0
            && let Some(since) = self.since.take()
        {
            self.elapsed += now.saturating_duration_since(since);
        }
    }
}

#[derive(Default)]
struct BlockingState {
    interval: Interval,
    current_turn: Option<String>,
    completed: HashMap<String, Duration>,
}

/// Thread-safe tracker measuring blocking wait time spent on guardian reviews.
#[derive(Default)]
pub struct ReviewBlocking {
    state: Mutex<BlockingState>,
}

impl ReviewBlocking {
    pub fn new() -> Self {
        Self::default()
    }

    /// Signals turn start.
    pub fn start_turn(&self, turn_id: &str) {
        let now = Instant::now();
        let mut st = self.state.lock().unwrap();
        if let Some(prev) = st.current_turn.replace(turn_id.to_string()) {
            let dur = st.interval.finish_turn(now);
            st.completed.insert(prev, dur);
        }
        st.interval.start_turn(now);
    }

    /// Signals that a review wait has entered blocking state.
    pub fn enter_review(&self) {
        let now = Instant::now();
        let mut st = self.state.lock().unwrap();
        st.interval.enter(now);
    }

    /// Signals that a review wait has exited blocking state.
    pub fn leave_review(&self) {
        let now = Instant::now();
        let mut st = self.state.lock().unwrap();
        st.interval.leave(now);
    }

    /// Signals turn finish, returning accumulated blocking duration.
    pub fn finish_turn(&self, turn_id: &str) -> Option<Duration> {
        let now = Instant::now();
        let mut st = self.state.lock().unwrap();
        if st.current_turn.as_deref() == Some(turn_id) {
            st.current_turn = None;
            Some(st.interval.finish_turn(now))
        } else {
            st.completed.remove(turn_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_review_blocking_duration_accumulation() {
        let tracker = ReviewBlocking::new();
        let turn = "turn_101";

        tracker.start_turn(turn);
        tracker.enter_review();
        std::thread::sleep(Duration::from_millis(10));
        tracker.leave_review();

        let dur = tracker.finish_turn(turn).expect("duration recorded");
        assert!(dur >= Duration::from_millis(8));
    }
}
