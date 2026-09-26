use crate::ids::TaskId;
use crate::scheduler::resources::{LockMode, ResourceKey};
use crate::state_machine::agent::AgentRole;
use std::collections::BTreeMap;

/// Entry representing a task ready for dispatch evaluation in the priority queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueTaskEntry {
    pub task_id: TaskId,
    pub base_priority: u32,
    pub waiting_ticks: u32,
    pub on_critical_path: bool,
    pub required_resources: Vec<(ResourceKey, LockMode)>,
    pub role: AgentRole,
}

impl QueueTaskEntry {
    pub fn new(
        task_id: TaskId,
        base_priority: u32,
        on_critical_path: bool,
        required_resources: Vec<(ResourceKey, LockMode)>,
        role: AgentRole,
    ) -> Self {
        Self {
            task_id,
            base_priority,
            waiting_ticks: 0,
            on_critical_path,
            required_resources,
            role,
        }
    }

    /// Calculate effective priority adding bounded waiting-age anti-starvation bonus (D-02).
    pub fn effective_priority(&self, interval: u32, increment: u32, max_bonus: u32) -> u32 {
        let age_bonus = self
            .waiting_ticks
            .checked_div(interval)
            .map(|ticks| (ticks * increment).min(max_bonus))
            .unwrap_or(0);
        self.base_priority.saturating_add(age_bonus)
    }
}

/// Priority dispatch queue featuring waiting-age anti-starvation and queue bypass (D-02).
#[derive(Debug, Clone)]
pub struct PriorityDispatchQueue {
    aging_interval: u32,
    aging_increment: u32,
    max_age_bonus: u32,
    starvation_threshold: u32,
    entries: BTreeMap<TaskId, QueueTaskEntry>,
}

impl Default for PriorityDispatchQueue {
    fn default() -> Self {
        Self::new(1, 10, 100, 10)
    }
}

impl PriorityDispatchQueue {
    pub fn new(
        aging_interval: u32,
        aging_increment: u32,
        max_age_bonus: u32,
        starvation_threshold: u32,
    ) -> Self {
        Self {
            aging_interval,
            aging_increment,
            max_age_bonus,
            starvation_threshold,
            entries: BTreeMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &BTreeMap<TaskId, QueueTaskEntry> {
        &self.entries
    }

    pub fn push(&mut self, entry: QueueTaskEntry) {
        self.entries.insert(entry.task_id, entry);
    }

    pub fn remove(&mut self, task_id: TaskId) -> Option<QueueTaskEntry> {
        self.entries.remove(&task_id)
    }

    pub fn get(&self, task_id: TaskId) -> Option<&QueueTaskEntry> {
        self.entries.get(&task_id)
    }

    /// Increment waiting ticks for a task when bypassed due to active resource conflicts.
    pub fn record_bypass(&mut self, task_id: TaskId) {
        if let Some(entry) = self.entries.get_mut(&task_id) {
            entry.waiting_ticks = entry.waiting_ticks.saturating_add(1);
        }
    }

    pub fn is_starving(&self, task_id: TaskId) -> bool {
        self.entries
            .get(&task_id)
            .is_some_and(|e| e.waiting_ticks >= self.starvation_threshold)
    }

    /// Select and remove the highest priority entry that satisfies the feasibility predicate.
    ///
    /// Comparison order:
    /// 1. Effective priority (descending)
    /// 2. Critical path flag (critical first)
    /// 3. TaskId ascending tie-breaker
    pub fn pop_highest_feasible<F>(&mut self, is_feasible: F) -> Option<QueueTaskEntry>
    where
        F: Fn(&QueueTaskEntry) -> bool,
    {
        let best_id = self
            .entries
            .values()
            .filter(|e| is_feasible(e))
            .max_by(|a, b| {
                let eff_a = a.effective_priority(
                    self.aging_interval,
                    self.aging_increment,
                    self.max_age_bonus,
                );
                let eff_b = b.effective_priority(
                    self.aging_interval,
                    self.aging_increment,
                    self.max_age_bonus,
                );

                eff_a
                    .cmp(&eff_b)
                    .then_with(|| a.on_critical_path.cmp(&b.on_critical_path))
                    .then_with(|| b.task_id.cmp(&a.task_id)) // smaller task_id is preferred in max_by
            })
            .map(|e| e.task_id)?;

        self.entries.remove(&best_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effective_priority_linear_aging() {
        let mut entry = QueueTaskEntry::new(
            TaskId::new(),
            100,
            false,
            Vec::new(),
            AgentRole::implementer(),
        );

        assert_eq!(entry.effective_priority(2, 5, 20), 100);
        entry.waiting_ticks = 1;
        assert_eq!(entry.effective_priority(2, 5, 20), 100);
        entry.waiting_ticks = 2;
        assert_eq!(entry.effective_priority(2, 5, 20), 105);
        entry.waiting_ticks = 6;
        assert_eq!(entry.effective_priority(2, 5, 20), 115);
        entry.waiting_ticks = 20;
        assert_eq!(entry.effective_priority(2, 5, 20), 120); // capped at max_bonus = 20
    }

    #[test]
    fn test_queue_bypass_dispatches_next_feasible() {
        let mut queue = PriorityDispatchQueue::new(1, 10, 50, 10);

        let t1 = TaskId::new();
        let t2 = TaskId::new();

        // High priority task T1 (requires resource X)
        queue.push(QueueTaskEntry::new(
            t1,
            200,
            false,
            vec![(
                ResourceKey::workspace(
                    "src/conflict",
                    crate::scheduler::resources::PathScope::Exact,
                ),
                LockMode::Exclusive,
            )],
            AgentRole::implementer(),
        ));

        // Lower priority task T2 (disjoint)
        queue.push(QueueTaskEntry::new(
            t2,
            100,
            false,
            Vec::new(),
            AgentRole::implementer(),
        ));

        // Feasibility check: T1 is resource-blocked, T2 is feasible
        let selected = queue.pop_highest_feasible(|e| e.task_id != t1);
        assert_eq!(selected.unwrap().task_id, t2);

        // Record bypass for T1
        queue.record_bypass(t1);
        assert_eq!(queue.get(t1).unwrap().waiting_ticks, 1);
        assert_eq!(queue.get(t1).unwrap().effective_priority(1, 10, 50), 210);
    }
}
