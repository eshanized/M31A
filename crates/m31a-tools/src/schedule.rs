//! Background prompt scheduler tool for one-shot timers and recurring cron triggers.
//!
//! Provides the `schedule` tool: registers one-shot timers (`duration_seconds`) or
//! recurring tasks (`cron_expression`) that deliver high-priority prompts back into
//! the agent turn loop.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tokio::time::{Duration, sleep};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleArgs {
    /// Message sent to the agent as a high-priority notification when the timer fires or cron triggers.
    pub prompt: String,
    /// Seconds to wait for a one-shot timer (mutually exclusive with cron_expression).
    #[serde(default)]
    pub duration_seconds: Option<u64>,
    /// Standard 5-field cron expression for a recurring schedule (e.g. '*/5 * * * *').
    #[serde(default)]
    pub cron_expression: Option<String>,
    /// Maximum number of triggers before the schedule terminates (default 1 for timers, unlimited for cron).
    #[serde(default)]
    pub max_iterations: Option<usize>,
    /// If true, keeps firing as a standalone daemon task even after the current turn concludes.
    #[serde(default)]
    pub is_daemon: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTaskInfo {
    pub task_id: String,
    pub prompt: String,
    pub schedule_type: String,
    pub created_at: DateTime<Utc>,
    pub trigger_count: usize,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleResult {
    pub task_id: String,
    pub message: String,
    pub status: String,
}

/// In-memory registry of scheduled tasks.
static SCHEDULE_REGISTRY: RwLock<Vec<ScheduledTaskInfo>> = RwLock::const_new(Vec::new());

/// Active task cancellation channels.
static CANCELLATION_HANDLES: RwLock<Option<HashMap<String, broadcast::Sender<()>>>> =
    RwLock::const_new(None);

/// Callback type for delivering scheduled prompts into the active agent turn loop.
pub type PromptDeliveryCallback = Arc<dyn Fn(String, String) + Send + Sync>;
static PROMPT_DELIVERY: RwLock<Option<PromptDeliveryCallback>> = RwLock::const_new(None);

/// Installs a listener for delivered scheduled prompts.
pub async fn register_prompt_delivery_callback(callback: PromptDeliveryCallback) {
    let mut lock = PROMPT_DELIVERY.write().await;
    *lock = Some(callback);
}

/// Parsed standard 5-field Cron Expression (minute, hour, day of month, month, day of week).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronExpression {
    pub minutes: HashSet<u32>,
    pub hours: HashSet<u32>,
    pub days_of_month: HashSet<u32>,
    pub months: HashSet<u32>,
    pub days_of_week: HashSet<u32>,
}

fn parse_cron_field(field: &str, min: u32, max: u32) -> Result<HashSet<u32>> {
    let mut values = HashSet::new();
    for part in field.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part == "*" {
            for v in min..=max {
                values.insert(v);
            }
        } else if let Some(step_str) = part.strip_prefix("*/") {
            let step: u32 = step_str
                .parse()
                .map_err(|_| anyhow!("Invalid cron step: {}", step_str))?;
            if step == 0 {
                return Err(anyhow!("Cron step cannot be 0"));
            }
            for v in (min..=max).step_by(step as usize) {
                values.insert(v);
            }
        } else if part.contains('/') {
            let sub_parts: Vec<&str> = part.split('/').collect();
            if sub_parts.len() != 2 {
                return Err(anyhow!("Invalid cron step expression: {}", part));
            }
            let step: u32 = sub_parts[1]
                .parse()
                .map_err(|_| anyhow!("Invalid step: {}", sub_parts[1]))?;
            if step == 0 {
                return Err(anyhow!("Cron step cannot be 0"));
            }
            let range_parts: Vec<&str> = sub_parts[0].split('-').collect();
            if range_parts.len() != 2 {
                return Err(anyhow!("Invalid cron range: {}", sub_parts[0]));
            }
            let start: u32 = range_parts[0]
                .parse()
                .map_err(|_| anyhow!("Invalid range start: {}", range_parts[0]))?;
            let end: u32 = range_parts[1]
                .parse()
                .map_err(|_| anyhow!("Invalid range end: {}", range_parts[1]))?;
            for v in (start..=end).step_by(step as usize) {
                if v >= min && v <= max {
                    values.insert(v);
                }
            }
        } else if part.contains('-') {
            let range_parts: Vec<&str> = part.split('-').collect();
            if range_parts.len() != 2 {
                return Err(anyhow!("Invalid cron range: {}", part));
            }
            let start: u32 = range_parts[0]
                .parse()
                .map_err(|_| anyhow!("Invalid range start: {}", range_parts[0]))?;
            let end: u32 = range_parts[1]
                .parse()
                .map_err(|_| anyhow!("Invalid range end: {}", range_parts[1]))?;
            if start > end {
                return Err(anyhow!("Invalid cron range (start > end): {}", part));
            }
            for v in start..=end {
                if v >= min && v <= max {
                    values.insert(v);
                }
            }
        } else {
            let val: u32 = part
                .parse()
                .map_err(|_| anyhow!("Invalid cron number: {}", part))?;
            if val < min || val > max {
                // In standard cron, Sunday can be 0 or 7
                if max == 6 && val == 7 {
                    values.insert(0);
                } else {
                    return Err(anyhow!(
                        "Cron value {} out of range ({} - {})",
                        val,
                        min,
                        max
                    ));
                }
            } else {
                values.insert(val);
            }
        }
    }
    if values.is_empty() {
        return Err(anyhow!("No valid values in cron field: {}", field));
    }
    Ok(values)
}

impl CronExpression {
    pub fn parse(expr: &str) -> Result<Self> {
        let fields: Vec<&str> = expr.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(anyhow!(
                "Invalid cron expression '{}': expected 5 fields (minute, hour, day, month, day-of-week), got {}",
                expr,
                fields.len()
            ));
        }

        let minutes = parse_cron_field(fields[0], 0, 59)?;
        let hours = parse_cron_field(fields[1], 0, 23)?;
        let days_of_month = parse_cron_field(fields[2], 1, 31)?;
        let months = parse_cron_field(fields[3], 1, 12)?;
        let days_of_week = parse_cron_field(fields[4], 0, 6)?;

        Ok(Self {
            minutes,
            hours,
            days_of_month,
            months,
            days_of_week,
        })
    }

    pub fn matches(&self, dt: &DateTime<Utc>) -> bool {
        let minute = dt.minute();
        let hour = dt.hour();
        let day = dt.day();
        let month = dt.month();
        let dow = dt.weekday().num_days_from_sunday();

        self.minutes.contains(&minute)
            && self.hours.contains(&hour)
            && self.days_of_month.contains(&day)
            && self.months.contains(&month)
            && self.days_of_week.contains(&dow)
    }

    pub fn next_occurrence(&self, from: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let from_ts = from.timestamp();
        let next_minute_ts = (from_ts / 60 + 1) * 60;
        let mut candidate = DateTime::from_timestamp(next_minute_ts, 0)?;

        // Search up to 525,600 minutes (1 year)
        for _ in 0..525_600 {
            if self.matches(&candidate) {
                return Some(candidate);
            }
            candidate += chrono::Duration::minutes(1);
        }
        None
    }
}

/// Executes the `schedule` tool.
pub async fn execute_schedule(args: &ScheduleArgs) -> Result<ScheduleResult> {
    if args.prompt.trim().is_empty() {
        return Err(anyhow!("The 'prompt' field cannot be empty."));
    }

    if args.duration_seconds.is_none() && args.cron_expression.is_none() {
        return Err(anyhow!(
            "Must provide either 'duration_seconds' for a one-shot timer or 'cron_expression' for recurring triggers."
        ));
    }

    if args.duration_seconds.is_some() && args.cron_expression.is_some() {
        return Err(anyhow!(
            "'duration_seconds' and 'cron_expression' are mutually exclusive."
        ));
    }

    // Pre-validate cron expression if provided
    let parsed_cron = if let Some(ref expr) = args.cron_expression {
        Some(CronExpression::parse(expr)?)
    } else {
        None
    };

    let task_id = format!("sched-{}", &Uuid::new_v4().to_string()[..8]);
    let prompt = args.prompt.clone();
    let duration_secs = args.duration_seconds;
    let cron_expr = args.cron_expression.clone();

    let schedule_type = if let Some(secs) = duration_secs {
        format!("one-shot ({}s)", secs)
    } else {
        format!("cron ({})", cron_expr.as_deref().unwrap_or(""))
    };

    let info = ScheduledTaskInfo {
        task_id: task_id.clone(),
        prompt: prompt.clone(),
        schedule_type: schedule_type.clone(),
        created_at: Utc::now(),
        trigger_count: 0,
        is_active: true,
    };

    {
        let mut reg = SCHEDULE_REGISTRY.write().await;
        reg.push(info);
    }

    let (cancel_tx, mut cancel_rx) = broadcast::channel(1);
    {
        let mut handles_lock = CANCELLATION_HANDLES.write().await;
        if handles_lock.is_none() {
            *handles_lock = Some(HashMap::new());
        }
        if let Some(ref mut map) = *handles_lock {
            map.insert(task_id.clone(), cancel_tx);
        }
    }

    let tid = task_id.clone();
    let prompt_msg = prompt.clone();

    if let Some(secs) = duration_secs {
        tokio::spawn(async move {
            tokio::select! {
                _ = sleep(Duration::from_secs(secs)) => {
                    tracing::info!(task_id = %tid, "Scheduled timer fired: triggering prompt");

                    let callback = {
                        let lock = PROMPT_DELIVERY.read().await;
                        lock.clone()
                    };

                    if let Some(cb) = callback {
                        cb(tid.clone(), prompt_msg);
                    }

                    // Mark inactive
                    let mut reg = SCHEDULE_REGISTRY.write().await;
                    if let Some(item) = reg.iter_mut().find(|t| t.task_id == tid) {
                        item.trigger_count += 1;
                        item.is_active = false;
                    }
                }
                _ = cancel_rx.recv() => {
                    tracing::info!(task_id = %tid, "Scheduled timer cancelled before firing");
                }
            }
        });
    } else if let Some(cron) = parsed_cron {
        let max_iter = args.max_iterations.unwrap_or(usize::MAX);
        tokio::spawn(async move {
            tracing::info!(task_id = %tid, max_iterations = max_iter, "Cron schedule active");
            let mut fired = 0;

            loop {
                let now = Utc::now();
                let next = match cron.next_occurrence(now) {
                    Some(t) => t,
                    None => {
                        tracing::warn!(task_id = %tid, "No future occurrences found for cron expression");
                        break;
                    }
                };

                let delay = (next - now).to_std().unwrap_or(Duration::from_millis(50));
                tokio::select! {
                    _ = sleep(delay) => {
                        tracing::info!(task_id = %tid, "Cron trigger fired: delivering prompt");

                        let callback = {
                            let lock = PROMPT_DELIVERY.read().await;
                            lock.clone()
                        };

                        if let Some(ref cb) = callback {
                            cb(tid.clone(), prompt_msg.clone());
                        }

                        fired += 1;
                        let mut should_stop = false;
                        {
                            let mut reg = SCHEDULE_REGISTRY.write().await;
                            if let Some(item) = reg.iter_mut().find(|t| t.task_id == tid) {
                                item.trigger_count += 1;
                                if item.trigger_count >= max_iter {
                                    item.is_active = false;
                                    should_stop = true;
                                }
                            }
                        }

                        if should_stop || fired >= max_iter {
                            break;
                        }
                    }
                    _ = cancel_rx.recv() => {
                        tracing::info!(task_id = %tid, "Cron schedule cancelled");
                        break;
                    }
                }
            }
        });
    }

    Ok(ScheduleResult {
        task_id: task_id.clone(),
        status: "active".to_string(),
        message: format!(
            "Scheduled task '{}' successfully registered ({}). Notification prompt will fire automatically.",
            task_id, schedule_type
        ),
    })
}

/// Cancels an active scheduled task by its task ID.
pub async fn cancel_scheduled_task(task_id: &str) -> bool {
    let sender = {
        let mut handles_lock = CANCELLATION_HANDLES.write().await;
        handles_lock.as_mut().and_then(|map| map.remove(task_id))
    };

    if let Some(tx) = sender {
        let _ = tx.send(());
        let mut reg = SCHEDULE_REGISTRY.write().await;
        if let Some(item) = reg.iter_mut().find(|t| t.task_id == task_id) {
            item.is_active = false;
        }
        true
    } else {
        false
    }
}

/// Retrieves list of all scheduled tasks.
pub async fn list_scheduled_tasks() -> Vec<ScheduledTaskInfo> {
    let lock = SCHEDULE_REGISTRY.read().await;
    lock.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cron_parsing_standard() {
        let cron = CronExpression::parse("*/5 0 * * *").unwrap();
        assert!(cron.minutes.contains(&0));
        assert!(cron.minutes.contains(&5));
        assert!(cron.minutes.contains(&55));
        assert!(!cron.minutes.contains(&1));
        assert!(cron.hours.contains(&0));
        assert!(!cron.hours.contains(&1));
    }

    #[test]
    fn test_cron_ranges_and_steps() {
        let cron = CronExpression::parse("10-20/5 9-17 1,15 * 1-5").unwrap();
        assert!(cron.minutes.contains(&10));
        assert!(cron.minutes.contains(&15));
        assert!(cron.minutes.contains(&20));
        assert!(!cron.minutes.contains(&12));
        assert!(cron.hours.contains(&9));
        assert!(cron.hours.contains(&17));
        assert!(!cron.hours.contains(&8));
        assert!(cron.days_of_month.contains(&1));
        assert!(cron.days_of_month.contains(&15));
        assert!(!cron.days_of_month.contains(&2));
        assert!(cron.days_of_week.contains(&1));
        assert!(cron.days_of_week.contains(&5));
        assert!(!cron.days_of_week.contains(&0)); // Sunday
    }

    #[test]
    fn test_cron_next_occurrence() {
        let cron = CronExpression::parse("30 14 * * *").unwrap();
        let from = DateTime::parse_from_rfc3339("2026-10-30T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let next = cron.next_occurrence(from).unwrap();
        assert_eq!(next.hour(), 14);
        assert_eq!(next.minute(), 30);
        assert_eq!(next.day(), 30);
    }

    #[tokio::test]
    async fn test_execute_schedule_and_cancel() {
        let args = ScheduleArgs {
            prompt: "Wakeup test".to_string(),
            duration_seconds: Some(60),
            cron_expression: None,
            max_iterations: Some(1),
            is_daemon: Some(false),
        };
        let res = execute_schedule(&args).await.unwrap();
        assert_eq!(res.status, "active");

        let tasks = list_scheduled_tasks().await;
        assert!(tasks.iter().any(|t| t.task_id == res.task_id));

        let cancelled = cancel_scheduled_task(&res.task_id).await;
        assert!(cancelled);

        let tasks_after = list_scheduled_tasks().await;
        let item = tasks_after
            .iter()
            .find(|t| t.task_id == res.task_id)
            .unwrap();
        assert!(!item.is_active);
    }
}
