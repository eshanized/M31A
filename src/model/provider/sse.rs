//! Asynchronous SSE stream accumulator, OpenAI-compatible JSON event parser, and HTTP error normalizer (D-02, D-03, D-04).

use regex::Regex;
use serde_json::Value;
use std::collections::BTreeMap;

use crate::model::types::{ModelError, ModelProposal, StreamChunk, TokenUsage, UsageSource};

/// Default `Retry-After` cooldown (seconds) when a 429 response carries no
/// parseable cooldown. ALGORITHMIC/transport heuristic: not operator policy
/// and not an operational timeout; configured timeouts are unaffected.
pub const RATE_LIMIT_DEFAULT_COOLDOWN_SECS: u64 = 30;

/// Accumulated in-progress tool call fragment.
#[derive(Debug, Default, Clone)]
struct AccumulatedToolCall {
    name: String,
    arguments: String,
}

/// Accumulator for streaming SSE frames from an OpenAI-compatible /chat/completions endpoint (D-02).
#[derive(Debug, Default)]
pub struct StreamAccumulator {
    content_accum: String,
    /// Reasoning/thinking deltas (`delta.reasoning_content`, emitted by
    /// reasoning models such as `nvidia/nemotron-3-ultra-550b-a55b`).
    /// Tracked separately from `content_accum`: reasoning is model-internal
    /// deliberation, never the authoritative answer, so it MUST NOT be mixed
    /// into the content stream that feeds proposal parsing.
    reasoning_accum: String,
    tool_calls: BTreeMap<usize, AccumulatedToolCall>,
    usage: Option<TokenUsage>,
    finish_reason: Option<String>,
    saw_done: bool,
}

impl StreamAccumulator {
    /// Create a new empty StreamAccumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Accumulated reasoning/thinking text observed on the stream so far.
    pub fn reasoning_text(&self) -> &str {
        &self.reasoning_accum
    }

    /// Process a single SSE event data frame.
    ///
    /// Returns a list of normalized `StreamChunk` events emitted by this frame.
    pub fn process_event(&mut self, event_data: &str) -> Result<Vec<StreamChunk>, ModelError> {
        let trimmed = event_data.trim();
        if trimmed == "[DONE]" {
            self.saw_done = true;
            return Ok(Vec::new());
        }

        let val: Value = serde_json::from_str(trimmed)
            .map_err(|e| ModelError::ProtocolViolation(format!("invalid SSE JSON frame: {e}")))?;

        let mut chunks = Vec::new();

        // Extract usage if present in the chunk
        if let Some(usage_val) = val.get("usage").filter(|v| !v.is_null()) {
            let prompt = usage_val
                .get("prompt_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let completion = usage_val
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let total = usage_val
                .get("total_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let reasoning = usage_val
                .get("completion_tokens_details")
                .and_then(|d| d.get("reasoning_tokens"))
                .and_then(|v| v.as_u64())
                .or_else(|| usage_val.get("reasoning_tokens").and_then(|v| v.as_u64()))
                .unwrap_or(0) as usize;

            let usage = TokenUsage::new(
                prompt,
                completion,
                total,
                reasoning,
                UsageSource::AuthoritativeProvider,
            );
            self.usage = Some(usage.clone());
            chunks.push(StreamChunk::UsageUpdate(usage));
        }

        // Extract choices deltas
        if let Some(choices) = val.get("choices").and_then(|c| c.as_array()) {
            for choice in choices {
                if let Some(finish) = choice.get("finish_reason").and_then(|f| f.as_str()) {
                    self.finish_reason = Some(finish.to_string());
                    chunks.push(StreamChunk::FinishReason(finish.to_string()));
                }

                if let Some(delta) = choice.get("delta") {
                    if let Some(content) = delta.get("content").and_then(|c| c.as_str())
                        && !content.is_empty()
                    {
                        self.content_accum.push_str(content);
                        chunks.push(StreamChunk::TextDelta(content.to_string()));
                    }

                    // Reasoning-model thinking deltas (e.g. Nemotron `reasoning_content`).
                    // Accumulated for observability only; never merged into the
                    // answer stream so JSON envelope parsing stays exact.
                    if let Some(reasoning) = delta.get("reasoning_content").and_then(|c| c.as_str())
                        && !reasoning.is_empty()
                    {
                        self.reasoning_accum.push_str(reasoning);
                    }

                    if let Some(tools) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                        for t in tools {
                            let index =
                                t.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                            let mut name_opt = None;
                            let mut args_delta = String::new();

                            let entry = self.tool_calls.entry(index).or_default();

                            if let Some(name) = t
                                .get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(|n| n.as_str())
                            {
                                entry.name.push_str(name);
                                name_opt = Some(name.to_string());
                            }

                            if let Some(args) = t
                                .get("function")
                                .and_then(|f| f.get("arguments"))
                                .and_then(|a| a.as_str())
                            {
                                entry.arguments.push_str(args);
                                args_delta = args.to_string();
                            }

                            chunks.push(StreamChunk::ToolCallDelta {
                                index,
                                name: name_opt,
                                arguments_delta: args_delta,
                            });
                        }
                    }
                }
            }
        }

        Ok(chunks)
    }

    /// Finalize and assemble the complete ModelProposal and TokenUsage (D-02, D-03).
    ///
    /// Validates JSON arguments before returning. Rejects empty streams with `ModelError::InvalidResponse`.
    pub fn finalize(self) -> Result<(ModelProposal, TokenUsage), ModelError> {
        let usage = self.usage.unwrap_or(TokenUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            reasoning_tokens: 0,
            source: UsageSource::Estimated,
        });

        // 1. Authoritative native tool calls
        if !self.tool_calls.is_empty() {
            let mut model_tool_calls = Vec::new();
            let mut complete_proposal = None;
            let mut ask_user_proposal = None;

            for (idx, tool) in self.tool_calls.into_iter() {
                if tool.name.is_empty() {
                    return Err(ModelError::InvalidResponse(
                        "empty tool call name in accumulated stream".to_string(),
                    ));
                }

                // Check for completion tools
                if tool.name == "complete"
                    || tool.name == "complete_task"
                    || tool.name == "finish"
                    || tool.name == "done"
                {
                    let params: Value =
                        serde_json::from_str(&tool.arguments).unwrap_or(Value::Null);
                    let summary = params
                        .get("summary")
                        .and_then(|s| s.as_str())
                        .unwrap_or(&tool.arguments)
                        .to_string();
                    complete_proposal = Some(ModelProposal::Complete {
                        summary,
                        artifacts: Vec::new(),
                    });
                    continue;
                }

                // Check for ask_user tool
                if tool.name == "ask_user" {
                    let params: Value =
                        serde_json::from_str(&tool.arguments).unwrap_or(Value::Null);
                    let question = params
                        .get("question")
                        .and_then(|s| s.as_str())
                        .unwrap_or(&tool.arguments)
                        .to_string();
                    let options: Vec<crate::model::types::UserOption> = params
                        .get("options")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|opt| {
                                    if let Some(s) = opt.as_str() {
                                        Some(crate::model::types::UserOption {
                                            id: s.to_string(),
                                            label: s.to_string(),
                                            description: None,
                                        })
                                    } else {
                                        serde_json::from_value(opt.clone()).ok()
                                    }
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    ask_user_proposal = Some(ModelProposal::AskUser { question, options });
                    continue;
                }

                let parameters = match crate::model::protocol::ToolCallProtocol::process_arguments(
                    &tool.arguments,
                    self.finish_reason.as_deref(),
                ) {
                    crate::model::protocol::ToolCallValidationOutcome::Valid(val) => val,
                    crate::model::protocol::ToolCallValidationOutcome::Recovered {
                        value,
                        repair_applied,
                    } => {
                        tracing::info!(
                            tool = %tool.name,
                            repair = %repair_applied,
                            "Truncated tool-call arguments safely recovered"
                        );
                        value
                    }
                    crate::model::protocol::ToolCallValidationOutcome::SyntaxError {
                        message,
                        raw_snippet,
                    } => {
                        tracing::warn!(
                            tool = %tool.name,
                            error = %message,
                            snippet_len = raw_snippet.len(),
                            "Malformed tool-call arguments syntax; passing structured error envelope to pipeline"
                        );
                        serde_json::json!({
                            "__malformed_error__": message,
                            "__raw_arguments__": raw_snippet,
                        })
                    }
                    crate::model::protocol::ToolCallValidationOutcome::UnrecoverableTruncation {
                        message,
                        raw_snippet,
                    } => {
                        tracing::warn!(
                            tool = %tool.name,
                            error = %message,
                            snippet_len = raw_snippet.len(),
                            "Unrecoverable truncated tool-call arguments; passing structured error envelope to pipeline"
                        );
                        serde_json::json!({
                            "__malformed_error__": message,
                            "__raw_arguments__": raw_snippet,
                            "__finish_reason__": "length",
                        })
                    }
                };

                model_tool_calls.push(crate::model::types::ModelToolCall {
                    id: format!("call_{}_{}", idx, uuid::Uuid::now_v7()),
                    name: tool.name,
                    arguments: parameters,
                });
            }

            if let Some(comp) = complete_proposal {
                return Ok((comp, usage));
            }
            if let Some(ask) = ask_user_proposal {
                return Ok((ask, usage));
            }
            if !model_tool_calls.is_empty() {
                return Ok((
                    ModelProposal::ToolCalls {
                        calls: model_tool_calls,
                    },
                    usage,
                ));
            }
        }

        // 2. Text completion with compatibility fallback envelope parsing
        if !self.content_accum.is_empty() {
            let trimmed = self.content_accum.trim();

            if let Some(action) = extract_embedded_action(trimmed) {
                return Ok((action, usage));
            }

            match crate::model::protocol::ToolCallProtocol::process_arguments(
                trimmed,
                self.finish_reason.as_deref(),
            ) {
                crate::model::protocol::ToolCallValidationOutcome::Valid(val)
                | crate::model::protocol::ToolCallValidationOutcome::Recovered {
                    value: val, ..
                } => {
                    if let Some(action) = try_parse_action_envelope(&val) {
                        return Ok((action, usage));
                    }
                }
                _ => {}
            }

            return Ok((
                ModelProposal::AssistantText {
                    content: self.content_accum,
                },
                usage,
            ));
        }

        // 3. Premature or empty termination without actions or content.
        // A reasoning-only stream (reasoning_content without content or tool
        // calls) is rejected here: thinking traces are not answers, and
        // treating them as completions would fabricate runtime evidence.
        Err(ModelError::InvalidResponse(
            "empty stream completion: no content or tool calls assembled".to_string(),
        ))
    }
}

/// Fallback parser for structured JSON action envelopes when native tool_calls are absent (D-03).
fn try_parse_action_envelope(val: &Value) -> Option<ModelProposal> {
    let obj = val.as_object()?;

    // 1. Tool calls array format: {"tool_calls": [{"function": {"name": "...", "arguments": ...}}]}
    if let Some(calls) = obj.get("tool_calls").and_then(|v| v.as_array()) {
        let mut parsed_calls = Vec::new();
        for (i, c) in calls.iter().enumerate() {
            if let Some(func) = c.get("function").and_then(|f| f.as_object())
                && let Some(name) = func.get("name").and_then(|n| n.as_str())
            {
                let params = func
                    .get("arguments")
                    .or_else(|| func.get("parameters"))
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                let normalized = match params {
                    Value::String(ref s) => serde_json::from_str::<Value>(s).unwrap_or(params),
                    other => other,
                };
                parsed_calls.push(crate::model::types::ModelToolCall {
                    id: c
                        .get("id")
                        .and_then(|id| id.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("call_{}_{}", i, uuid::Uuid::now_v7())),
                    name: name.to_string(),
                    arguments: normalized,
                });
            }
        }
        if !parsed_calls.is_empty() {
            return Some(ModelProposal::ToolCalls {
                calls: parsed_calls,
            });
        }
    }

    // 1b. Format: {"type": "ask_user", "question": "...", "options": [...]}
    if let (Some(t), Some(question)) = (
        obj.get("type").and_then(|v| v.as_str()),
        obj.get("question").and_then(|v| v.as_str()),
    ) && (t == "ask_user" || t == "ask")
    {
        let options: Vec<crate::model::types::UserOption> = obj
            .get("options")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|opt| {
                        if let Some(s) = opt.as_str() {
                            Some(crate::model::types::UserOption {
                                id: s.to_string(),
                                label: s.to_string(),
                                description: None,
                            })
                        } else {
                            serde_json::from_value(opt.clone()).ok()
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        return Some(ModelProposal::AskUser {
            question: question.to_string(),
            options,
        });
    }

    // 2. Function call wrapper: {"function": {"name": "...", "arguments": ...}}
    if let Some(func) = obj.get("function").and_then(|v| v.as_object())
        && let Some(name) = func.get("name").and_then(|v| v.as_str())
    {
        let params = func
            .get("arguments")
            .or_else(|| func.get("parameters"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let normalized_params = match params {
            Value::String(ref s) => serde_json::from_str::<Value>(s).unwrap_or(params),
            other => other,
        };
        return Some(ModelProposal::ToolCalls {
            calls: vec![crate::model::types::ModelToolCall {
                id: format!("call_0_{}", uuid::Uuid::now_v7()),
                name: name.to_string(),
                arguments: normalized_params,
            }],
        });
    }

    // 3. Format: {"type": "handoff", "target_role": "...", "reason": "..."}
    if let (Some(t), Some(role), Some(reason)) = (
        obj.get("type").and_then(|v| v.as_str()),
        obj.get("target_role").and_then(|v| v.as_str()),
        obj.get("reason").and_then(|v| v.as_str()),
    ) && t == "handoff"
    {
        return Some(ModelProposal::Handoff {
            target_role: role.to_string(),
            reason: reason.to_string(),
        });
    }

    // 4. Format: {"type": "complete", "summary": "..."}
    if let (Some(t), Some(summary)) = (
        obj.get("type").and_then(|v| v.as_str()),
        obj.get("summary").and_then(|v| v.as_str()),
    ) && t == "complete"
    {
        return Some(ModelProposal::Complete {
            summary: summary.to_string(),
            artifacts: Vec::new(),
        });
    }

    // 5. Generalized action: tool name in "tool", "tool_name", or "name"
    let tool_name = obj
        .get("tool")
        .or_else(|| obj.get("tool_name"))
        .or_else(|| obj.get("name"))
        .and_then(|v| v.as_str());

    if let Some(tool) = tool_name {
        // Special case: if tool is "complete", normalize to Complete proposal
        if tool == "complete" || tool == "complete_task" {
            let summary = obj
                .get("summary")
                .or_else(|| obj.get("parameters").and_then(|p| p.get("summary")))
                .or_else(|| obj.get("arguments").and_then(|a| a.get("summary")))
                .and_then(|v| v.as_str())
                .unwrap_or("task completed");
            return Some(ModelProposal::Complete {
                summary: summary.to_string(),
                artifacts: Vec::new(),
            });
        }

        let raw_params = obj
            .get("parameters")
            .or_else(|| obj.get("arguments"))
            .or_else(|| obj.get("args"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        let normalized_params = match raw_params {
            Value::String(ref s) => serde_json::from_str::<Value>(s).unwrap_or(raw_params),
            other => other,
        };

        return Some(ModelProposal::ToolCalls {
            calls: vec![crate::model::types::ModelToolCall {
                id: format!("call_0_{}", uuid::Uuid::now_v7()),
                name: tool.to_string(),
                arguments: normalized_params,
            }],
        });
    }

    None
}

/// Extract inner JSON content from markdown codeblock if present.
fn extract_json_codeblock(text: &str) -> Option<String> {
    if let Some(start) = text.find("```json") {
        let after = &text[start + 7..];
        if let Some(end) = after.find("```") {
            return Some(after[..end].trim().to_string());
        }
    } else if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        if let Some(end) = after.find("```") {
            return Some(after[..end].trim().to_string());
        }
    }
    None
}

/// Scan text for JSON blocks or embedded action envelopes (markdown or prose).
fn extract_embedded_action(text: &str) -> Option<ModelProposal> {
    // 1. Markdown codeblock
    if let Some(code) = extract_json_codeblock(text)
        && let Ok(parsed) = serde_json::from_str::<Value>(&code)
        && let Some(action) = try_parse_action_envelope(&parsed)
    {
        return Some(action);
    }

    // 2. Direct JSON string
    let trimmed = text.trim();
    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed)
        && let Some(action) = try_parse_action_envelope(&parsed)
    {
        return Some(action);
    }

    // 3. Scan text for any candidate JSON object { ... }
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let start = i;
            let mut depth = 0;
            let mut in_string = false;
            let mut escape = false;
            let mut j = i;
            while j < bytes.len() {
                let b = bytes[j];
                if escape {
                    escape = false;
                } else if b == b'\\' && in_string {
                    escape = true;
                } else if b == b'"' {
                    in_string = !in_string;
                } else if !in_string {
                    if b == b'{' {
                        depth += 1;
                    } else if b == b'}' {
                        depth -= 1;
                        if depth == 0 {
                            if let Ok(candidate) = std::str::from_utf8(&bytes[start..=j])
                                && let Ok(parsed) = serde_json::from_str::<Value>(candidate)
                                && let Some(action) = try_parse_action_envelope(&parsed)
                            {
                                return Some(action);
                            }
                            break;
                        }
                    }
                }
                j += 1;
            }
        }
        i += 1;
    }

    None
}

/// Normalize HTTP status codes and response bodies into typed `ModelError` variants (D-04).
///
/// Strips sensitive credentials (API keys, Authorization headers) from error messages.
pub fn normalize_http_error(status: u16, body: &str) -> ModelError {
    let sanitized_body = sanitize_credentials(body);

    match status {
        401 | 403 => ModelError::AuthenticationFailed,
        429 => {
            let cooldown_secs = extract_cooldown_seconds(&sanitized_body)
                .unwrap_or(RATE_LIMIT_DEFAULT_COOLDOWN_SECS);
            ModelError::RateLimited { cooldown_secs }
        }
        400 => {
            let lower = sanitized_body.to_lowercase();
            if lower.contains("context length")
                || lower.contains("maximum tokens")
                || lower.contains("context_length_exceeded")
                || lower.contains("max_tokens")
            {
                let (requested, capacity) = extract_context_window_numbers(&lower);
                ModelError::ContextWindowExhausted {
                    requested,
                    capacity,
                }
            } else {
                ModelError::Http {
                    status,
                    message: sanitized_body,
                }
            }
        }
        _ => ModelError::Http {
            status,
            message: sanitized_body,
        },
    }
}

/// Redact credentials and auth headers from error bodies.
fn sanitize_credentials(text: &str) -> String {
    let bearer_re = Regex::new(r"(?i)bearer\s+[A-Za-z0-9_\-\.]+").unwrap();
    let redacted = bearer_re.replace_all(text, "Bearer [REDACTED]");

    let nvapi_re = Regex::new(r"nvapi-[A-Za-z0-9_\-]+").unwrap();
    let redacted = nvapi_re.replace_all(&redacted, "nvapi-[REDACTED]");

    let key_re = Regex::new(r#"(?i)(api[_-]?key["']?\s*[:=]\s*["']?)[A-Za-z0-9_\-]+"#).unwrap();
    let redacted = key_re.replace_all(&redacted, "${1}[REDACTED]");

    redacted.to_string()
}

/// Extract cooldown seconds from a 429 rate limit response.
fn extract_cooldown_seconds(body: &str) -> Option<u64> {
    if let Ok(val) = serde_json::from_str::<Value>(body) {
        if let Some(secs) = val.get("retry_after").and_then(|v| v.as_u64()) {
            return Some(secs);
        }
        if let Some(secs) = val
            .get("error")
            .and_then(|e| e.get("cooldown"))
            .and_then(|v| v.as_u64())
        {
            return Some(secs);
        }
    }

    let re = Regex::new(r"(?i)retry[- ]after[:\s]+(\d+)").unwrap();
    if let Some(caps) = re.captures(body)
        && let Some(m) = caps.get(1)
        && let Ok(secs) = m.as_str().parse::<u64>()
    {
        return Some(secs);
    }

    let re_sec = Regex::new(r"(\d+)\s*(?:s|sec|seconds)").unwrap();
    if let Some(caps) = re_sec.captures(body)
        && let Some(m) = caps.get(1)
        && let Ok(secs) = m.as_str().parse::<u64>()
    {
        return Some(secs);
    }

    None
}

/// Extract requested and capacity token counts from a context length error.
fn extract_context_window_numbers(lower_body: &str) -> (usize, usize) {
    let re = Regex::new(r"(\d+)\s*tokens?").unwrap();
    let matches: Vec<usize> = re
        .captures_iter(lower_body)
        .filter_map(|cap| cap.get(1).and_then(|m| m.as_str().parse::<usize>().ok()))
        .collect();

    if matches.len() >= 2 {
        (matches[0], matches[1])
    } else {
        (0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_single_and_multi_chunk_text_streaming() {
        let mut accumulator = StreamAccumulator::new();

        let event1 = json!({
            "choices": [{
                "index": 0,
                "delta": { "content": "Hello " }
            }]
        })
        .to_string();

        let event2 = json!({
            "choices": [{
                "index": 0,
                "delta": { "content": "world!" },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        })
        .to_string();

        let chunks1 = accumulator.process_event(&event1).unwrap();
        assert_eq!(chunks1.len(), 1);
        assert_eq!(chunks1[0], StreamChunk::TextDelta("Hello ".to_string()));

        let chunks2 = accumulator.process_event(&event2).unwrap();
        assert_eq!(chunks2.len(), 3);
        assert_eq!(
            chunks2[0],
            StreamChunk::UsageUpdate(TokenUsage::new(
                10,
                5,
                15,
                0,
                UsageSource::AuthoritativeProvider
            ))
        );
        assert_eq!(chunks2[1], StreamChunk::FinishReason("stop".to_string()));
        assert_eq!(chunks2[2], StreamChunk::TextDelta("world!".to_string()));

        let (proposal, usage) = accumulator.finalize().unwrap();
        assert_eq!(
            proposal,
            ModelProposal::AssistantText {
                content: "Hello world!".to_string(),
            }
        );
        assert_eq!(usage.total_tokens, 15);
        assert_eq!(usage.source, UsageSource::AuthoritativeProvider);
    }

    #[test]
    fn test_multi_chunk_fragmented_tool_call_accumulation() {
        let mut accumulator = StreamAccumulator::new();

        let event1 = json!({
            "choices": [{
                "index": 0,
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "function": { "name": "file_" }
                    }]
                }
            }]
        })
        .to_string();

        let event2 = json!({
            "choices": [{
                "index": 0,
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "function": { "name": "read", "arguments": "{\"path\":" }
                    }]
                }
            }]
        })
        .to_string();

        let event3 = json!({
            "choices": [{
                "index": 0,
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "function": { "arguments": "\"src/main.rs\"}" }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        })
        .to_string();

        accumulator.process_event(&event1).unwrap();
        accumulator.process_event(&event2).unwrap();
        accumulator.process_event(&event3).unwrap();
        accumulator.process_event("[DONE]").unwrap();

        let (proposal, _) = accumulator.finalize().unwrap();
        match proposal {
            ModelProposal::ToolCalls { calls } => {
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].name, "file_read");
                assert_eq!(calls[0].arguments, json!({ "path": "src/main.rs" }));
            }
            _ => panic!("expected tool calls proposal"),
        }
    }

    #[test]
    fn test_malformed_tool_call_json_arguments() {
        let mut accumulator = StreamAccumulator::new();

        let event = json!({
            "choices": [{
                "index": 0,
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "function": { "name": "do_work", "arguments": "{invalid_json" }
                    }]
                }
            }]
        })
        .to_string();

        accumulator.process_event(&event).unwrap();
        let (proposal, _) = accumulator.finalize().unwrap();
        match proposal {
            ModelProposal::ToolCalls { calls } => {
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].name, "do_work");
                assert!(calls[0].arguments.get("__malformed_error__").is_some());
                assert_eq!(
                    calls[0]
                        .arguments
                        .get("__raw_arguments__")
                        .and_then(|v| v.as_str()),
                    Some("{invalid_json")
                );
            }
            _ => panic!("expected tool calls proposal with malformed error payload"),
        }
    }

    #[test]
    fn test_empty_stream_done_handling() {
        let mut accumulator = StreamAccumulator::new();
        accumulator.process_event("[DONE]").unwrap();
        let err = accumulator.finalize().unwrap_err();
        assert!(matches!(err, ModelError::InvalidResponse(_)));
    }

    #[test]
    fn test_fallback_envelope_parsing() {
        let mut accumulator = StreamAccumulator::new();

        let event = json!({
            "choices": [{
                "index": 0,
                "delta": {
                    "content": "{\"tool\": \"cargo_check\", \"parameters\": {\"workspace\": true}}"
                }
            }]
        })
        .to_string();

        accumulator.process_event(&event).unwrap();
        let (proposal, _) = accumulator.finalize().unwrap();
        match proposal {
            ModelProposal::ToolCalls { calls } => {
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].name, "cargo_check");
                assert_eq!(calls[0].arguments, json!({ "workspace": true }));
            }
            _ => panic!("expected tool calls proposal from fallback envelope"),
        }
    }

    #[test]
    fn test_reasoning_content_tracked_without_polluting_answer() {
        let mut accumulator = StreamAccumulator::new();

        // Reasoning-model frame shape (verified live against
        // nvidia/nemotron-3-ultra-550b-a55b): thinking arrives as
        // delta.reasoning_content, the answer as delta.content.
        let thinking = json!({
            "choices": [{
                "index": 0,
                "delta": { "role": "assistant", "reasoning_content": "Let me think... " }
            }]
        })
        .to_string();
        let answer = json!({
            "choices": [{
                "index": 0,
                "delta": { "role": "assistant", "content": "{\"tool\": \"cargo_check\", \"parameters\": {}}" },
                "finish_reason": "stop"
            }]
        })
        .to_string();

        accumulator.process_event(&thinking).unwrap();
        assert_eq!(accumulator.reasoning_text(), "Let me think... ");
        accumulator.process_event(&answer).unwrap();

        let (proposal, _) = accumulator.finalize().unwrap();
        match proposal {
            ModelProposal::ToolCalls { calls } => {
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].name, "cargo_check");
            }
            other => panic!("expected tool calls proposal, got {other:?}"),
        }
    }

    #[test]
    fn test_reasoning_only_stream_rejected_as_empty() {
        let mut accumulator = StreamAccumulator::new();
        let thinking = json!({
            "choices": [{
                "index": 0,
                "delta": { "reasoning_content": "thinking without answering" },
                "finish_reason": "stop"
            }]
        })
        .to_string();
        accumulator.process_event(&thinking).unwrap();
        assert!(!accumulator.reasoning_text().is_empty());
        let err = accumulator.finalize().unwrap_err();
        assert!(matches!(err, ModelError::InvalidResponse(_)));
    }

    #[test]
    fn test_normalize_http_errors() {
        assert_eq!(
            normalize_http_error(401, "unauthorized"),
            ModelError::AuthenticationFailed
        );
        assert_eq!(
            normalize_http_error(403, "forbidden"),
            ModelError::AuthenticationFailed
        );

        let err_429 = normalize_http_error(429, "Rate limit reached. Retry after 42 seconds.");
        assert_eq!(err_429, ModelError::RateLimited { cooldown_secs: 42 });

        let err_ctx = normalize_http_error(
            400,
            "This model's maximum context length is 8192 tokens. However, you requested 9500 tokens.",
        );
        match err_ctx {
            ModelError::ContextWindowExhausted {
                requested,
                capacity,
            } => {
                assert!(requested > 0 || capacity > 0);
            }
            _ => panic!("expected ContextWindowExhausted, got {err_ctx:?}"),
        }

        let err_500 = normalize_http_error(500, "internal server error");
        assert!(matches!(err_500, ModelError::Http { status: 500, .. }));
    }

    #[test]
    fn test_normalize_http_error_credential_redaction() {
        let body_with_creds =
            "Error with Bearer nvapi-12345secret and api_key: topsecrettoken occurred";
        let err = normalize_http_error(500, body_with_creds);
        match err {
            ModelError::Http { message, .. } => {
                assert!(!message.contains("nvapi-12345secret"));
                assert!(!message.contains("topsecrettoken"));
                assert!(message.contains("[REDACTED]"));
            }
            _ => panic!("expected Http error"),
        }
    }
}
