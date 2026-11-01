//! OpenTelemetry distributed tracing and span export integration.
//!
//! Emits structured trace spans for agent turns, tool invocations, and model completions.

use anyhow::{Result, bail};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime};

pub const W3C_TRACEPARENT_HEADER: &str = "traceparent";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceContext {
    pub version: String,
    pub trace_id: String,
    pub span_id: String,
    pub trace_flags: String,
}

impl TraceContext {
    /// Generates a new unique W3C trace context.
    pub fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let trace_id = format!("{:032x}", now);
        let span_id = format!("{:016x}", (now >> 64) ^ (now & 0xFFFFFFFFFFFFFFFF));

        Self {
            version: "00".to_string(),
            trace_id,
            span_id,
            trace_flags: "01".to_string(), // Sampled
        }
    }

    /// Formats as standard W3C `traceparent` header string: `00-<trace_id>-<span_id>-<trace_flags>`.
    pub fn format_traceparent(&self) -> String {
        format!(
            "{}-{}-{}-{}",
            self.version, self.trace_id, self.span_id, self.trace_flags
        )
    }

    /// Parses a standard W3C `traceparent` header.
    pub fn parse_traceparent(header: &str) -> Result<Self> {
        let parts: Vec<&str> = header.trim().split('-').collect();
        if parts.len() != 4 {
            bail!(
                "Invalid W3C traceparent header: expected 4 hyphen-separated fields, got {}",
                parts.len()
            );
        }

        let version = parts[0];
        let trace_id = parts[1];
        let span_id = parts[2];
        let trace_flags = parts[3];

        if version.len() != 2
            || trace_id.len() != 32
            || span_id.len() != 16
            || trace_flags.len() != 2
        {
            bail!("Invalid W3C traceparent field lengths");
        }

        Ok(Self {
            version: version.to_string(),
            trace_id: trace_id.to_string(),
            span_id: span_id.to_string(),
            trace_flags: trace_flags.to_string(),
        })
    }

    /// Derives a child span context with the same trace ID and a new span ID.
    pub fn create_child_span(&self) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let new_span_id = format!("{:016x}", now & 0xFFFFFFFFFFFFFFFF);

        Self {
            version: self.version.clone(),
            trace_id: self.trace_id.clone(),
            span_id: new_span_id,
            trace_flags: self.trace_flags.clone(),
        }
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SpanTimer {
    pub name: String,
    pub trace_context: TraceContext,
    pub start: Instant,
    pub tags: HashMap<String, String>,
}

impl SpanTimer {
    pub fn start(name: &str, ctx: TraceContext) -> Self {
        Self {
            name: name.to_string(),
            trace_context: ctx,
            start: Instant::now(),
            tags: HashMap::new(),
        }
    }

    pub fn with_tag(mut self, key: &str, value: &str) -> Self {
        self.tags.insert(key.to_string(), value.to_string());
        self
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_context_generation_and_formatting() {
        let ctx = TraceContext::new();
        let formatted = ctx.format_traceparent();
        assert!(formatted.starts_with("00-"));
        assert_eq!(formatted.len(), 55); // 2 + 1 + 32 + 1 + 16 + 1 + 2 = 55 chars

        let parsed = TraceContext::parse_traceparent(&formatted).unwrap();
        assert_eq!(parsed, ctx);
    }

    #[test]
    fn test_child_span_creation() {
        let parent = TraceContext::new();
        let child = parent.create_child_span();

        assert_eq!(child.trace_id, parent.trace_id);
        assert_ne!(child.span_id, parent.span_id);
    }
}
