//! HTTP transport client for NVIDIA NIM and OpenAI-compatible inference endpoints.
//!
//! Handles request formatting, SSE token streaming, tool call parsing, and connection pooling.

use anyhow::{Result, anyhow, bail};
use futures_util::StreamExt;
use m31a_core::types::{ChatMessage, FunctionCall, TokenUsage, ToolCall, ToolDefinition};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

/// Tunable request options for NVIDIA NIM inference.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NimRequestOptions {
    /// Bounded maximum output tokens.
    pub max_tokens: Option<u32>,
    /// Reasoning effort level (e.g., "low", "medium", "high").
    pub reasoning_effort: Option<String>,
    /// Bounded ceiling on reasoning tokens. Must be less than `max_tokens`.
    pub reasoning_budget: Option<i32>,
    /// Sampling temperature (0.0 to 2.0).
    pub temperature: Option<f32>,
    /// Nucleus sampling probability threshold (0.0 to 1.0).
    pub top_p: Option<f32>,
    /// Tool choice mode: "auto", "required", "none", or a specific function name.
    pub tool_choice: Option<String>,
    /// Low-level template parameters (e.g., `enable_thinking`, `force_nonempty_content`).
    pub chat_template_kwargs: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Clone)]
pub struct NvidiaNimClient {
    client: reqwest::Client,
    api_base: String,
    api_key: String,
    model: String,
    options: NimRequestOptions,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamOptions {
    pub include_usage: bool,
}

#[derive(Serialize, Debug)]
pub struct ChatCompletionRequest<'a> {
    pub model: &'a str,
    pub messages: &'a [ChatMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<&'a [ToolDefinition]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_budget: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_template_kwargs: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<&'a str>,
}

#[derive(Deserialize, Debug)]
pub struct StreamChunk {
    pub choices: Option<Vec<StreamChoice>>,
    pub usage: Option<StreamUsage>,
    pub error: Option<StreamErrorPayload>,
}

#[derive(Deserialize, Debug)]
pub struct StreamErrorPayload {
    pub message: Option<String>,
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub code: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
pub struct StreamErrorEnvelope {
    pub error: Option<StreamErrorPayload>,
}

#[derive(Deserialize, Debug)]
pub struct StreamChoice {
    pub delta: Option<StreamDelta>,
    #[allow(dead_code)]
    pub finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct StreamDelta {
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<StreamToolCall>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StreamToolCall {
    pub index: usize,
    pub id: Option<String>,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub call_type: Option<String>,
    pub function: Option<StreamFunctionCall>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StreamFunctionCall {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct StreamUsage {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

#[derive(Deserialize, Debug)]
pub struct NonStreamChatResponse {
    pub choices: Option<Vec<NonStreamChoice>>,
    pub usage: Option<StreamUsage>,
    pub error: Option<StreamErrorPayload>,
}

#[derive(Deserialize, Debug)]
pub struct NonStreamChoice {
    pub message: Option<NonStreamMessage>,
    #[allow(dead_code)]
    pub finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct NonStreamMessage {
    #[allow(dead_code)]
    pub role: Option<String>,
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<StreamToolCall>>,
}

pub type StreamCallback = Arc<dyn Fn(&str) + Send + Sync>;

impl NvidiaNimClient {
    pub fn new(
        api_base: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "User-Agent",
            HeaderValue::from_static("M31A/0.1.13 (Linux x86_64)"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .tcp_nodelay(true)
            .tcp_keepalive(Some(Duration::from_secs(60)))
            .connect_timeout(Duration::from_secs(15))
            .pool_max_idle_per_host(16)
            .pool_idle_timeout(Some(Duration::from_secs(90)))
            .build()
            .unwrap_or_default();

        Self {
            client,
            api_base: api_base.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            model: model.into(),
            options: NimRequestOptions::default(),
        }
    }

    pub fn with_model(&self, model: impl Into<String>) -> Self {
        let mut clone = self.clone();
        clone.model = model.into();
        clone
    }

    pub fn with_options(mut self, options: NimRequestOptions) -> Self {
        self.options = options;
        self
    }

    pub fn options(&self) -> &NimRequestOptions {
        &self.options
    }

    pub fn options_mut(&mut self) -> &mut NimRequestOptions {
        &mut self.options
    }

    pub fn with_reasoning_effort(mut self, effort: Option<String>) -> Self {
        self.options.reasoning_effort = effort;
        self
    }

    pub fn with_reasoning_budget(mut self, budget: Option<i32>) -> Self {
        self.options.reasoning_budget = budget;
        self
    }

    pub fn with_max_tokens(mut self, max: Option<u32>) -> Self {
        self.options.max_tokens = max;
        self
    }

    pub fn with_temperature(mut self, temp: Option<f32>) -> Self {
        self.options.temperature = temp;
        self
    }

    pub fn with_top_p(mut self, top_p: Option<f32>) -> Self {
        self.options.top_p = top_p;
        self
    }

    pub fn with_tool_choice(mut self, choice: Option<String>) -> Self {
        self.options.tool_choice = choice;
        self
    }

    pub fn with_chat_template_kwargs(
        mut self,
        kwargs: Option<HashMap<String, serde_json::Value>>,
    ) -> Self {
        self.options.chat_template_kwargs = kwargs;
        self
    }

    pub fn reasoning_effort(&self) -> Option<&str> {
        self.options.reasoning_effort.as_deref()
    }

    pub fn reasoning_budget(&self) -> Option<i32> {
        self.options.reasoning_budget
    }

    pub fn max_tokens(&self) -> Option<u32> {
        self.options.max_tokens
    }

    pub fn temperature(&self) -> Option<f32> {
        self.options.temperature
    }

    pub fn top_p(&self) -> Option<f32> {
        self.options.top_p
    }

    pub fn tool_choice(&self) -> Option<&str> {
        self.options.tool_choice.as_deref()
    }

    pub fn chat_template_kwargs(&self) -> Option<&HashMap<String, serde_json::Value>> {
        self.options.chat_template_kwargs.as_ref()
    }

    /// Model identifier used for requests.
    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn api_base(&self) -> &str {
        &self.api_base
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Builds a typed, model-profiled chat completion request.
    pub fn build_request_body<'a>(
        &'a self,
        messages: &'a [ChatMessage],
        tools: Option<&'a [ToolDefinition]>,
        stream: bool,
    ) -> Result<ChatCompletionRequest<'a>> {
        let is_nemotron_ultra = self.model == "nvidia/nemotron-3-ultra-550b-a55b"
            || self.model.contains("nemotron-3-ultra");
        let is_reasoning_model = is_nemotron_ultra
            || self.model.contains("deepseek")
            || self.model.contains("reasoning")
            || self.model.contains("thinking")
            || self.model.contains("r1");

        // 1. Validate reasoning_effort
        if let Some(ref effort) = self.options.reasoning_effort {
            match effort.as_str() {
                "none" | "medium" | "high" => {}
                other => bail!(
                    "Invalid reasoning_effort '{other}': must be one of 'none', 'medium', 'high'"
                ),
            }
        }

        // 2. Validate reasoning_budget
        if let Some(budget) = self.options.reasoning_budget
            && !(-1..=32768).contains(&budget)
        {
            bail!("Invalid reasoning_budget ({budget}): must be between -1 and 32768");
        }

        // 3. Validate max_tokens
        if let Some(max_tok) = self.options.max_tokens
            && !(1..=32768).contains(&max_tok)
        {
            bail!("Invalid max_tokens ({max_tok}): must be between 1 and 32768");
        }

        // 4. Validate reasoning budget against max_tokens if both are configured
        if let (Some(max_tok), Some(budget)) =
            (self.options.max_tokens, self.options.reasoning_budget)
            && budget > 0
            && budget >= max_tok as i32
        {
            bail!(
                "Invalid Nemotron reasoning configuration: reasoning_budget ({}) must be strictly less than max_tokens ({})",
                budget,
                max_tok
            );
        }

        // 5. Validate temperature
        if let Some(temp) = self.options.temperature
            && !(0.0..=2.0).contains(&temp)
        {
            bail!("Invalid temperature ({temp}): must be between 0.0 and 2.0");
        }

        // 6. Validate top_p
        if let Some(top_p) = self.options.top_p
            && !(0.0..=1.0).contains(&top_p)
        {
            bail!("Invalid top_p ({top_p}): must be between 0.0 and 1.0");
        }

        let mut chat_kwargs = self.options.chat_template_kwargs.clone();
        let effort_str = self.options.reasoning_effort.as_deref();

        if is_nemotron_ultra || is_reasoning_model {
            let kwargs = chat_kwargs.get_or_insert_with(HashMap::new);
            let enable = effort_str != Some("none");
            kwargs
                .entry("enable_thinking".to_string())
                .or_insert(serde_json::Value::Bool(enable));

            // NVIDIA Nemotron Ultra tool calling requires force_nonempty_content
            if tools.is_some() && !tools.unwrap().is_empty() {
                kwargs
                    .entry("force_nonempty_content".to_string())
                    .or_insert(serde_json::Value::Bool(true));
            }
        }

        let temp = self
            .options
            .temperature
            .or(if is_nemotron_ultra || is_reasoning_model {
                Some(0.6)
            } else {
                Some(0.2)
            });

        let tool_choice_val = if tools.is_some() {
            if let Some(ref tc) = self.options.tool_choice {
                match tc.as_str() {
                    "auto" => Some(serde_json::Value::String("auto".to_string())),
                    "required" => Some(serde_json::Value::String("required".to_string())),
                    "none" => Some(serde_json::Value::String("none".to_string())),
                    other => {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(other) {
                            Some(parsed)
                        } else {
                            Some(serde_json::json!({
                                "type": "function",
                                "function": { "name": other }
                            }))
                        }
                    }
                }
            } else {
                Some(serde_json::Value::String("auto".to_string()))
            }
        } else {
            None
        };

        let stream_opts = if stream {
            Some(StreamOptions {
                include_usage: true,
            })
        } else {
            None
        };

        Ok(ChatCompletionRequest {
            model: &self.model,
            messages,
            tools,
            tool_choice: tool_choice_val,
            stream,
            stream_options: stream_opts,
            temperature: temp,
            top_p: self.options.top_p,
            max_tokens: self.options.max_tokens,
            reasoning_budget: self.options.reasoning_budget,
            chat_template_kwargs: chat_kwargs,
            reasoning_effort: effort_str,
        })
    }

    /// Checks connectivity to the upstream NIM endpoint with a bounded 5s timeout.
    pub async fn check_connection(&self) -> Result<()> {
        let endpoint = format!("{}/models", self.api_base);
        let resp = self
            .client
            .get(&endpoint)
            .bearer_auth(&self.api_key)
            .timeout(Duration::from_secs(5))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else if status.as_u16() == 401 || status.as_u16() == 403 {
            bail!("Authentication failed (HTTP {})", status);
        } else if status.as_u16() == 429 {
            bail!("Rate limit reached (HTTP 429)");
        } else if status.is_server_error() {
            bail!("Provider server error (HTTP {})", status);
        } else {
            bail!("Connection check returned HTTP {}", status);
        }
    }

    /// Verifies genuine availability of `self.model` on the upstream NIM provider.
    ///
    /// 1. Queries the `/models` endpoint to inspect the live model catalog.
    /// 2. If the catalog is returned with a `data` list, verifies that `self.model` is present.
    /// 3. If model listing is inconclusive (e.g. non-standard response without `data`),
    ///    uses an explicitly bounded 1-token inference probe to confirm capability for this model.
    pub async fn test_model_availability(&self) -> Result<()> {
        let endpoint = format!("{}/models", self.api_base);
        let resp = self
            .client
            .get(&endpoint)
            .bearer_auth(&self.api_key)
            .timeout(Duration::from_secs(5))
            .send()
            .await?;

        let status = resp.status();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            bail!("Authentication failed (HTTP {})", status);
        } else if status.as_u16() == 429 {
            bail!("Rate limit reached (HTTP 429)");
        } else if status.is_server_error() {
            bail!("Provider server error (HTTP {})", status);
        } else if !status.is_success() {
            bail!("Model availability check returned HTTP {}", status);
        }

        if let Ok(json) = resp.json::<serde_json::Value>().await
            && let Some(arr) = json.get("data").and_then(|v| v.as_array())
        {
            let target = self.model.trim();
            let found = arr.iter().any(|item| {
                item.get("id")
                    .and_then(|v| v.as_str())
                    .is_some_and(|id| id.trim() == target || id.trim().eq_ignore_ascii_case(target))
            });
            if found {
                return Ok(());
            } else {
                bail!(
                    "Model '{}' is not available on NVIDIA NIM endpoint",
                    self.model
                );
            }
        }

        // Fallback: where provider cannot reliably establish availability from model metadata,
        // use an explicitly bounded 1-token inference probe.
        self.probe_model_inference().await
    }

    /// Performs an explicitly bounded 1-token inference probe to verify model availability.
    pub async fn probe_model_inference(&self) -> Result<()> {
        let endpoint = format!("{}/chat/completions", self.api_base);
        let probe_msg = [ChatMessage::user("ping")];
        let mut probe_client = self.clone();
        probe_client.options.max_tokens = Some(1);
        probe_client.options.reasoning_budget = None;
        probe_client.options.reasoning_effort = Some("none".to_string());
        let body = probe_client.build_request_body(&probe_msg, None, false)?;

        let retry_policy = crate::retry::RetryPolicy::default();
        let mut attempt = 0;

        loop {
            let resp = probe_client
                .client
                .post(&endpoint)
                .bearer_auth(&probe_client.api_key)
                .header(CONTENT_TYPE, "application/json")
                .timeout(Duration::from_secs(10))
                .json(&body)
                .send()
                .await?;

            let status = resp.status();
            if status.is_success() {
                return Ok(());
            }

            if retry_policy.is_retryable(status) && attempt < 2 {
                let delay = retry_policy.calculate_delay(attempt, None);
                tokio::time::sleep(delay).await;
                attempt += 1;
                continue;
            }

            let error_text = resp.text().await.unwrap_or_default();
            bail!(
                "Model '{}' inference probe failed (HTTP {}): {}",
                probe_client.model,
                status,
                error_text
            );
        }
    }

    /// Performs streaming chat completion using hardened SSE frame parsing.
    pub async fn stream_chat(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
        on_content_chunk: Option<StreamCallback>,
        on_reasoning_chunk: Option<StreamCallback>,
    ) -> Result<(ChatMessage, TokenUsage)> {
        let endpoint = format!("{}/chat/completions", self.api_base);
        let mut active_options = self.options.clone();
        let retry_policy = crate::retry::RetryPolicy::default();
        let mut attempt = 0;

        loop {
            let mut client_clone = self.clone();
            client_clone.options = active_options.clone();
            let request_body = client_clone.build_request_body(messages, tools, true)?;

            let req_start = std::time::Instant::now();
            let res = self
                .client
                .post(&endpoint)
                .header(AUTHORIZATION, format!("Bearer {}", self.api_key))
                .json(&request_body)
                .timeout(Duration::from_secs(120))
                .send()
                .await;

            let elapsed_ms = req_start.elapsed().as_millis() as u64;

            let response = match res {
                Ok(resp) => {
                    let status = resp.status();
                    let headers = resp.headers().clone();
                    let debug_ctx = crate::debug_context::ResponseDebugContext::extract(
                        &headers,
                        Some(status.as_u16()),
                        elapsed_ms,
                    );

                    if status.is_success() {
                        tracing::debug!("NVIDIA NIM success [{}]", debug_ctx.format_summary());
                        resp
                    } else {
                        let err_body = resp.text().await.unwrap_or_default();

                        // Detect upstream vLLM V2 thinking_token_budget incompatibility
                        if (status.as_u16() == 400 || status.is_server_error())
                            && err_body.contains("thinking_token_budget")
                            && active_options.reasoning_budget.is_some_and(|b| b > 0)
                        {
                            warn!(
                                attempt,
                                "Upstream NVIDIA NIM vLLM V2 runner does not support thinking_token_budget; retrying with reasoning_budget=-1 (unconstrained)"
                            );
                            active_options.reasoning_budget = Some(-1);
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            attempt += 1;
                            continue;
                        }

                        if retry_policy.is_retryable(status) && attempt < retry_policy.max_retries {
                            let delay =
                                retry_policy.calculate_delay(attempt, debug_ctx.retry_after_secs);
                            warn!(
                                attempt,
                                status = %status,
                                delay_ms = delay.as_millis(),
                                diag = %debug_ctx.format_summary(),
                                "Transient NVIDIA NIM error, retrying..."
                            );
                            tokio::time::sleep(delay).await;
                            attempt += 1;
                            continue;
                        }

                        return Err(anyhow!(
                            "NVIDIA NIM API Error ({status}) [{}]: {err_body}",
                            debug_ctx.format_summary()
                        ));
                    }
                }
                Err(err) => {
                    if attempt < retry_policy.max_retries {
                        let delay = retry_policy.calculate_delay(attempt, None);
                        warn!(
                            attempt,
                            error = %err,
                            delay_ms = delay.as_millis(),
                            "Network connection failed, retrying..."
                        );
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(anyhow!(
                        "Failed to connect to NVIDIA NIM endpoint {}: {}",
                        endpoint,
                        err
                    ));
                }
            };

            // Stream SSE chunks
            let mut stream = response.bytes_stream();
            let mut accumulator = SseStreamAccumulator::new();
            let mut stream_err = None;

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Err(e) = accumulator.feed_chunk(
                            &chunk,
                            on_content_chunk.as_ref(),
                            on_reasoning_chunk.as_ref(),
                        ) {
                            stream_err = Some(e);
                            break;
                        }
                        if accumulator.stream_done {
                            break;
                        }
                    }
                    Err(e) => {
                        stream_err = Some(anyhow!("Error reading streaming response chunk: {}", e));
                        break;
                    }
                }
            }

            if let Some(err) = stream_err {
                let err_str = err.to_string();

                // If upstream returned 500 error in stream with positive reasoning_budget,
                // fallback to reasoning_budget=-1 (unconstrained) for the vLLM V2 runner
                if (err_str.contains("500") || err_str.contains("thinking_token_budget"))
                    && active_options.reasoning_budget.is_some_and(|b| b > 0)
                {
                    warn!(
                        attempt,
                        "Upstream NVIDIA NIM returned stream 500 with positive reasoning_budget; retrying with reasoning_budget=-1 (unconstrained)"
                    );
                    active_options.reasoning_budget = Some(-1);
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    attempt += 1;
                    continue;
                }

                let is_transient = err_str.contains("500")
                    || err_str.contains("502")
                    || err_str.contains("503")
                    || err_str.contains("504")
                    || err_str.contains("429")
                    || err_str.contains("overloaded")
                    || err_str.contains("temporarily");

                if is_transient && attempt < retry_policy.max_retries {
                    let delay = retry_policy.calculate_delay(attempt, None);
                    warn!(
                        attempt,
                        err = %err_str,
                        delay_ms = delay.as_millis(),
                        "Transient NVIDIA NIM SSE stream error, retrying..."
                    );
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                    continue;
                }

                return Err(err);
            }

            accumulator.flush_tail(on_content_chunk.as_ref(), on_reasoning_chunk.as_ref())?;
            return Ok(accumulator.finalize());
        }
    }

    /// Non-streaming chat completion.
    pub async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<(ChatMessage, TokenUsage)> {
        let endpoint = format!("{}/chat/completions", self.api_base);
        let mut active_options = self.options.clone();
        let retry_policy = crate::retry::RetryPolicy::default();
        let mut attempt = 0;

        loop {
            let mut client_clone = self.clone();
            client_clone.options = active_options.clone();
            let request_body = client_clone.build_request_body(messages, tools, false)?;

            let req_start = std::time::Instant::now();
            let res = self
                .client
                .post(&endpoint)
                .header(AUTHORIZATION, format!("Bearer {}", self.api_key))
                .json(&request_body)
                .timeout(Duration::from_secs(120))
                .send()
                .await;

            let elapsed_ms = req_start.elapsed().as_millis() as u64;

            match res {
                Ok(resp) => {
                    let status = resp.status();
                    let headers = resp.headers().clone();
                    let debug_ctx = crate::debug_context::ResponseDebugContext::extract(
                        &headers,
                        Some(status.as_u16()),
                        elapsed_ms,
                    );

                    if status.is_success() {
                        tracing::debug!("NVIDIA NIM success [{}]", debug_ctx.format_summary());
                        let parsed = resp.json::<NonStreamChatResponse>().await?;
                        if let Some(err) = parsed.error {
                            bail!(
                                "NVIDIA NIM error: {}",
                                err.message.unwrap_or_else(|| "Unknown error".to_string())
                            );
                        }

                        let choice = parsed.choices.and_then(|mut c| {
                            if c.is_empty() {
                                None
                            } else {
                                Some(c.remove(0))
                            }
                        });
                        let msg = choice.and_then(|c| c.message);

                        let content = msg.as_ref().and_then(|m| m.content.clone());
                        let reasoning = msg.as_ref().and_then(|m| m.reasoning_content.clone());
                        let tool_calls = msg.and_then(|m| {
                            m.tool_calls.map(|tcs| {
                                tcs.into_iter()
                                    .map(|tc| {
                                        let func_name = tc
                                            .function
                                            .as_ref()
                                            .and_then(|f| f.name.clone())
                                            .unwrap_or_default();
                                        let func_args = tc
                                            .function
                                            .as_ref()
                                            .and_then(|f| f.arguments.clone())
                                            .unwrap_or_default();
                                        ToolCall {
                                            id: tc
                                                .id
                                                .unwrap_or_else(|| format!("call_{}", tc.index)),
                                            call_type: "function".to_string(),
                                            function: FunctionCall {
                                                name: func_name,
                                                arguments: func_args,
                                            },
                                        }
                                    })
                                    .collect()
                            })
                        });

                        let mut assistant = ChatMessage::assistant(content, tool_calls);
                        if let Some(r) = reasoning
                            && !r.is_empty()
                        {
                            assistant.reasoning_content = Some(r);
                        }

                        let usage = TokenUsage {
                            prompt_tokens: parsed
                                .usage
                                .as_ref()
                                .and_then(|u| u.prompt_tokens)
                                .unwrap_or(0),
                            completion_tokens: parsed
                                .usage
                                .as_ref()
                                .and_then(|u| u.completion_tokens)
                                .unwrap_or(0),
                            total_tokens: parsed
                                .usage
                                .as_ref()
                                .and_then(|u| u.total_tokens)
                                .unwrap_or(0),
                            reasoning_tokens: 0,
                        };

                        return Ok((assistant, usage));
                    }

                    let err_body = resp.text().await.unwrap_or_default();

                    // Detect upstream vLLM V2 thinking_token_budget incompatibility
                    if (status.as_u16() == 400 || status.is_server_error())
                        && err_body.contains("thinking_token_budget")
                        && active_options.reasoning_budget.is_some_and(|b| b > 0)
                    {
                        warn!(
                            attempt,
                            "Upstream NVIDIA NIM vLLM V2 runner does not support thinking_token_budget; retrying with reasoning_budget=-1 (unconstrained)"
                        );
                        active_options.reasoning_budget = Some(-1);
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        attempt += 1;
                        continue;
                    }

                    if retry_policy.is_retryable(status) && attempt < retry_policy.max_retries {
                        let delay =
                            retry_policy.calculate_delay(attempt, debug_ctx.retry_after_secs);
                        warn!(
                            attempt,
                            status = %status,
                            delay_ms = delay.as_millis(),
                            diag = %debug_ctx.format_summary(),
                            "Transient NVIDIA NIM error, retrying..."
                        );
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }

                    return Err(anyhow!(
                        "NVIDIA NIM API Error ({status}) [{}]: {err_body}",
                        debug_ctx.format_summary()
                    ));
                }
                Err(err) => {
                    if attempt < retry_policy.max_retries {
                        let delay = retry_policy.calculate_delay(attempt, None);
                        warn!(
                            attempt,
                            error = %err,
                            delay_ms = delay.as_millis(),
                            "Network connection failed, retrying..."
                        );
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(anyhow!(
                        "Failed to connect to NVIDIA NIM endpoint {}: {}",
                        endpoint,
                        err
                    ));
                }
            }
        }
    }

    /// Computes dense vector embeddings for input strings via `/v1/embeddings`.
    pub async fn create_embeddings(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        let endpoint = format!("{}/embeddings", self.api_base);
        let request_body = serde_json::json!({
            "model": self.model,
            "input": inputs,
            "input_type": "query"
        });

        let resp = self
            .client
            .post(&endpoint)
            .header(AUTHORIZATION, format!("Bearer {}", self.api_key))
            .json(&request_body)
            .timeout(Duration::from_secs(60))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_body = resp.text().await.unwrap_or_default();
            bail!("NVIDIA NIM Embedding API Error ({status}): {err_body}");
        }

        let body: serde_json::Value = resp.json().await?;
        let mut results = Vec::new();
        if let Some(arr) = body.get("data").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(emb_arr) = item.get("embedding").and_then(|v| v.as_array()) {
                    let vec: Vec<f32> = emb_arr
                        .iter()
                        .filter_map(|n| n.as_f64().map(|f| f as f32))
                        .collect();
                    results.push(vec);
                }
            }
        }

        Ok(results)
    }

    /// Performs active health status check against the configured model endpoint.
    pub async fn probe_model_health(&self) -> Result<crate::health_monitor::HealthStatus> {
        let monitor = crate::health_monitor::LocalModelHealthMonitor::new(&self.api_base);
        Ok(monitor.check_health().await)
    }
}

/// Hardened SSE stream accumulator that buffers bytes across arbitrary chunk boundaries
/// and parses both intermediate frames and final trailing frames through a unified path.
#[derive(Default)]
pub struct SseStreamAccumulator {
    pub accumulated_content: String,
    pub accumulated_reasoning: String,
    pub tool_calls_map: HashMap<usize, (String, String, String)>, // index -> (id, name, args)
    pub usage: TokenUsage,
    pub stream_done: bool,
    byte_buffer: Vec<u8>,
}

impl SseStreamAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds raw bytes from an HTTP chunk into the accumulator.
    pub fn feed_chunk(
        &mut self,
        chunk: &[u8],
        on_content_chunk: Option<&StreamCallback>,
        on_reasoning_chunk: Option<&StreamCallback>,
    ) -> Result<()> {
        self.byte_buffer.extend_from_slice(chunk);

        let mut last_processed = 0;
        let mut i = 0;
        while i < self.byte_buffer.len() {
            if self.byte_buffer[i] == b'\n' {
                let line_str = std::str::from_utf8(&self.byte_buffer[last_processed..i])
                    .map_err(|e| anyhow!("Invalid UTF-8 in SSE stream: {}", e))?
                    .to_string();

                let is_done = self.process_line(&line_str, on_content_chunk, on_reasoning_chunk)?;
                last_processed = i + 1;
                if is_done {
                    self.stream_done = true;
                    break;
                }
            }
            i += 1;
        }

        if last_processed > 0 {
            self.byte_buffer.drain(..last_processed);
        }

        Ok(())
    }

    /// Processes any trailing bytes at the end of the HTTP transport.
    /// Trailing frames without a final newline are parsed through the exact same logic.
    pub fn flush_tail(
        &mut self,
        on_content_chunk: Option<&StreamCallback>,
        on_reasoning_chunk: Option<&StreamCallback>,
    ) -> Result<()> {
        if !self.stream_done && !self.byte_buffer.is_empty() {
            let line_bytes = std::mem::take(&mut self.byte_buffer);
            let line_str = std::str::from_utf8(&line_bytes)
                .map_err(|e| anyhow!("Invalid UTF-8 in trailing SSE frame: {}", e))?;
            if !line_str.trim().is_empty() {
                let _ = self.process_line(line_str, on_content_chunk, on_reasoning_chunk)?;
            }
        }
        Ok(())
    }

    /// Single unified line/frame parser.
    pub fn process_line(
        &mut self,
        line: &str,
        on_content_chunk: Option<&StreamCallback>,
        on_reasoning_chunk: Option<&StreamCallback>,
    ) -> Result<bool> {
        let line = line.trim_end_matches('\r').trim();

        if line.is_empty() || line.starts_with(':') {
            return Ok(false);
        }

        // Direct JSON provider error detection
        if line.starts_with('{')
            && let Ok(err_obj) = serde_json::from_str::<StreamErrorEnvelope>(line)
            && let Some(err) = err_obj.error
        {
            bail!(
                "NVIDIA NIM stream error: {}",
                err.message.unwrap_or_else(|| "Unknown error".to_string())
            );
        }

        let raw_data = match line.strip_prefix("data:") {
            Some(d) => d.trim(),
            None => match line.strip_prefix("data") {
                Some(d) => d.trim(),
                None => {
                    if line.contains("\"error\"") {
                        bail!("NVIDIA NIM error in stream: {}", line);
                    }
                    return Ok(false);
                }
            },
        };

        if raw_data == "[DONE]" {
            self.stream_done = true;
            return Ok(true);
        }

        let parsed: StreamChunk = serde_json::from_str(raw_data)
            .map_err(|e| anyhow!("Failed to parse SSE JSON payload ({e}): {raw_data}"))?;

        if let Some(err) = parsed.error {
            bail!(
                "NVIDIA NIM streaming error ({:?}): {}",
                err.code,
                err.message.unwrap_or_else(|| "Unknown error".to_string())
            );
        }

        if let Some(u) = parsed.usage {
            self.usage.prompt_tokens = u.prompt_tokens.unwrap_or(0);
            self.usage.completion_tokens = u.completion_tokens.unwrap_or(0);
            self.usage.total_tokens = u.total_tokens.unwrap_or(0);
        }

        if let Some(choices) = parsed.choices {
            for choice in choices {
                if let Some(delta) = choice.delta {
                    if let Some(text) = delta.content
                        && !text.is_empty()
                    {
                        self.accumulated_content.push_str(&text);
                        if let Some(cb) = on_content_chunk {
                            cb(&text);
                        }
                    }

                    if let Some(reasoning) = delta.reasoning_content
                        && !reasoning.is_empty()
                    {
                        self.accumulated_reasoning.push_str(&reasoning);
                        if let Some(cb) = on_reasoning_chunk {
                            cb(&reasoning);
                        }
                    }

                    if let Some(t_calls) = delta.tool_calls {
                        for tc in t_calls {
                            let entry = self.tool_calls_map.entry(tc.index).or_insert_with(|| {
                                (
                                    tc.id
                                        .clone()
                                        .unwrap_or_else(|| format!("call_{}", tc.index)),
                                    String::new(),
                                    String::new(),
                                )
                            });

                            if let Some(id) = tc.id {
                                entry.0 = id;
                            }
                            if let Some(func) = tc.function {
                                if let Some(name) = func.name {
                                    entry.1.push_str(&name);
                                }
                                if let Some(args) = func.arguments {
                                    entry.2.push_str(&args);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    /// Converts accumulated state into `(ChatMessage, TokenUsage)`.
    pub fn finalize(mut self) -> (ChatMessage, TokenUsage) {
        let mut final_tool_calls = None;
        if !self.tool_calls_map.is_empty() {
            let mut indices: Vec<usize> = self.tool_calls_map.keys().copied().collect();
            indices.sort_unstable();

            let mut calls = Vec::new();
            for idx in indices {
                if let Some((id, name, arguments)) = self.tool_calls_map.remove(&idx) {
                    calls.push(ToolCall {
                        id,
                        call_type: "function".to_string(),
                        function: FunctionCall { name, arguments },
                    });
                }
            }
            final_tool_calls = Some(calls);
        }

        let content_opt = if self.accumulated_content.is_empty() {
            None
        } else {
            Some(self.accumulated_content)
        };

        let mut assistant_msg = ChatMessage::assistant(content_opt, final_tool_calls);
        if !self.accumulated_reasoning.is_empty() {
            assistant_msg.reasoning_content = Some(self.accumulated_reasoning);
        }

        (assistant_msg, self.usage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn test_api_key() -> String {
        m31a_core::resolve_stored_api_key()
            .unwrap_or_else(|| "nvapi-runtime-integration-token".to_string())
    }

    #[test]
    fn test_sse_split_utf8_multibyte_code_points() {
        let mut accumulator = SseStreamAccumulator::new();

        // 4-byte UTF-8 emoji: 🚀 = [0xF0, 0x9F, 0x9A, 0x80]
        let frame_prefix = b"data: {\"choices\":[{\"delta\":{\"content\":\"Rocket: ";
        let rocket_bytes = [0xF0, 0x9F, 0x9A, 0x80];
        let frame_suffix = b"\"}}]}\n";

        // Chunk 1: prefix + first 2 bytes of emoji
        let mut chunk1 = frame_prefix.to_vec();
        chunk1.extend_from_slice(&rocket_bytes[0..2]);

        // Chunk 2: remaining 2 bytes of emoji + suffix
        let mut chunk2 = rocket_bytes[2..4].to_vec();
        chunk2.extend_from_slice(frame_suffix);

        accumulator.feed_chunk(&chunk1, None, None).unwrap();
        accumulator.feed_chunk(&chunk2, None, None).unwrap();
        accumulator.flush_tail(None, None).unwrap();

        let (msg, _) = accumulator.finalize();
        assert_eq!(msg.content.as_deref(), Some("Rocket: 🚀"));
    }

    #[test]
    fn test_sse_crlf_and_lf_line_endings() {
        let mut accumulator = SseStreamAccumulator::new();

        let chunk = b"data: {\"choices\":[{\"delta\":{\"content\":\"A\"}}]}\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"B\"}}]}\n";
        accumulator.feed_chunk(chunk, None, None).unwrap();
        accumulator.flush_tail(None, None).unwrap();

        let (msg, _) = accumulator.finalize();
        assert_eq!(msg.content.as_deref(), Some("AB"));
    }

    #[test]
    fn test_sse_multiple_frames_in_one_chunk() {
        let mut accumulator = SseStreamAccumulator::new();

        let chunk = b"data: {\"choices\":[{\"delta\":{\"content\":\"Hello \"}}]}\ndata: {\"choices\":[{\"delta\":{\"content\":\"world\"}}]}\ndata: [DONE]\n";
        accumulator.feed_chunk(chunk, None, None).unwrap();
        accumulator.flush_tail(None, None).unwrap();

        assert!(accumulator.stream_done);
        let (msg, _) = accumulator.finalize();
        assert_eq!(msg.content.as_deref(), Some("Hello world"));
    }

    #[test]
    fn test_sse_final_frame_without_trailing_newline() {
        let mut accumulator = SseStreamAccumulator::new();

        let chunk1 = b"data: {\"choices\":[{\"delta\":{\"content\":\"first \"}}]}\n";
        let chunk2_no_newline = b"data: {\"choices\":[{\"delta\":{\"content\":\"second\"}}]}";

        accumulator.feed_chunk(chunk1, None, None).unwrap();
        accumulator
            .feed_chunk(chunk2_no_newline, None, None)
            .unwrap();
        accumulator.flush_tail(None, None).unwrap();

        let (msg, _) = accumulator.finalize();
        assert_eq!(msg.content.as_deref(), Some("first second"));
    }

    #[test]
    fn test_sse_final_frame_tool_call_without_newline() {
        let mut accumulator = SseStreamAccumulator::new();

        let tool_chunk_no_newline = br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"get_weather","arguments":"{\"city\":\"Tokyo\"}"}}]}}]}"#;
        accumulator
            .feed_chunk(tool_chunk_no_newline, None, None)
            .unwrap();
        accumulator.flush_tail(None, None).unwrap();

        let (msg, _) = accumulator.finalize();
        let tools = msg.tool_calls.expect("Must parse tool call in tail");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].id, "call_abc");
        assert_eq!(tools[0].function.name, "get_weather");
        assert_eq!(tools[0].function.arguments, "{\"city\":\"Tokyo\"}");
    }

    #[test]
    fn test_sse_tool_call_incremental_assembly() {
        let mut accumulator = SseStreamAccumulator::new();

        let chunk1 = br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_123","function":{"name":"calc"}}]}}]}"#;
        let chunk2 = br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"a\": 1"}}]}}]}"#;
        let chunk3 = br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":", \"b\": 2}"}}]}}]}"#;

        for c in [&chunk1[..], &chunk2[..], &chunk3[..]] {
            let mut with_nl = c.to_vec();
            with_nl.push(b'\n');
            accumulator.feed_chunk(&with_nl, None, None).unwrap();
        }
        accumulator.flush_tail(None, None).unwrap();

        let (msg, _) = accumulator.finalize();
        let calls = msg.tool_calls.unwrap();
        assert_eq!(calls[0].id, "call_123");
        assert_eq!(calls[0].function.name, "calc");
        assert_eq!(calls[0].function.arguments, "{\"a\": 1, \"b\": 2}");
    }

    #[test]
    fn test_sse_reasoning_and_content_separation() {
        let mut accumulator = SseStreamAccumulator::new();
        let reasoning_received = Arc::new(AtomicUsize::new(0));
        let content_received = Arc::new(AtomicUsize::new(0));

        let r_cb = Arc::clone(&reasoning_received);
        let c_cb = Arc::clone(&content_received);

        let cb_reasoning: StreamCallback = Arc::new(move |_| {
            r_cb.fetch_add(1, Ordering::SeqCst);
        });
        let cb_content: StreamCallback = Arc::new(move |_| {
            c_cb.fetch_add(1, Ordering::SeqCst);
        });

        let chunk = b"data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"Thinking step 1...\"}}]}\ndata: {\"choices\":[{\"delta\":{\"content\":\"Final Answer.\"}}]}\n";
        accumulator
            .feed_chunk(chunk, Some(&cb_content), Some(&cb_reasoning))
            .unwrap();
        accumulator.flush_tail(None, None).unwrap();

        let (msg, _) = accumulator.finalize();
        assert_eq!(msg.reasoning_content.as_deref(), Some("Thinking step 1..."));
        assert_eq!(msg.content.as_deref(), Some("Final Answer."));
        assert_eq!(reasoning_received.load(Ordering::SeqCst), 1);
        assert_eq!(content_received.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_sse_usage_extraction() {
        let mut accumulator = SseStreamAccumulator::new();

        let chunk = b"data: {\"choices\":[{\"delta\":{\"content\":\"Done\"}}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"total_tokens\":15}}\n";
        accumulator.feed_chunk(chunk, None, None).unwrap();
        accumulator.flush_tail(None, None).unwrap();

        let (_, usage) = accumulator.finalize();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        assert_eq!(usage.total_tokens, 15);
    }

    #[test]
    fn test_sse_malformed_json_returns_error() {
        let mut accumulator = SseStreamAccumulator::new();

        let chunk = b"data: {this is not valid json}\n";
        let res = accumulator.feed_chunk(chunk, None, None);
        assert!(res.is_err(), "Malformed JSON in data frame must return Err");
    }

    #[test]
    fn test_sse_provider_error_payload_returns_error() {
        let mut accumulator = SseStreamAccumulator::new();

        let chunk = b"{\"error\":{\"message\":\"Rate limit exceeded\",\"type\":\"rate_limit_error\",\"code\":429}}\n";
        let res = accumulator.feed_chunk(chunk, None, None);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Rate limit exceeded"));

        // Also test error in data frame
        let mut acc2 = SseStreamAccumulator::new();
        let chunk_data_err =
            b"data: {\"error\":{\"message\":\"Context length exceeded\",\"code\":400}}\n";
        let res2 = acc2.feed_chunk(chunk_data_err, None, None);
        assert!(res2.is_err());
        assert!(
            res2.unwrap_err()
                .to_string()
                .contains("Context length exceeded")
        );
    }

    #[test]
    fn test_nemotron_ultra_request_serialization() {
        let client = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            test_api_key(),
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_max_tokens(Some(1024))
        .with_reasoning_budget(Some(512))
        .with_reasoning_effort(Some("medium".to_string()))
        .with_tool_choice(Some("required".to_string()))
        .with_top_p(Some(0.95));

        let tools = vec![ToolDefinition {
            tool_type: "function".to_string(),
            function: m31a_core::types::FunctionDefinition {
                name: "test_tool".to_string(),
                description: "Test".to_string(),
                parameters: serde_json::json!({}),
            },
        }];

        let messages = vec![ChatMessage::user("Hi")];
        let req = client
            .build_request_body(&messages, Some(&tools), true)
            .expect("build request");

        let json = serde_json::to_value(&req).expect("serialize request to json");

        assert_eq!(json["model"], "nvidia/nemotron-3-ultra-550b-a55b");
        assert_eq!(json["max_tokens"], 1024);
        assert_eq!(json["reasoning_budget"], 512);
        assert_eq!(json["reasoning_effort"], "medium");
        assert!((json["top_p"].as_f64().unwrap() - 0.95).abs() < 1e-4);
        assert!((json["temperature"].as_f64().unwrap() - 0.6).abs() < 1e-4);
        assert_eq!(json["tool_choice"], "required");
        assert_eq!(json["stream"], true);
        assert_eq!(json["stream_options"]["include_usage"], true);

        assert_eq!(
            json["chat_template_kwargs"]["enable_thinking"],
            serde_json::Value::Bool(true)
        );
        assert_eq!(
            json["chat_template_kwargs"]["force_nonempty_content"],
            serde_json::Value::Bool(true)
        );
    }

    #[test]
    fn test_reasoning_budget_exceeding_max_tokens_rejected() {
        let client = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            test_api_key(),
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_max_tokens(Some(256))
        .with_reasoning_budget(Some(512));

        let messages = vec![ChatMessage::user("Hi")];
        let res = client.build_request_body(&messages, None, true);
        assert!(res.is_err(), "Budget >= max_tokens must be rejected");
        assert!(res.unwrap_err().to_string().contains("reasoning_budget"));
    }

    #[test]
    fn test_nemotron_request_serialization_reasoning_disabled() {
        let client = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            test_api_key(),
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_reasoning_effort(Some("none".to_string()));

        let messages = vec![ChatMessage::user("Hi")];
        let req = client.build_request_body(&messages, None, false).unwrap();
        let json = serde_json::to_value(&req).unwrap();

        assert_eq!(json["reasoning_effort"], "none");
        assert_eq!(
            json["chat_template_kwargs"]["enable_thinking"],
            serde_json::Value::Bool(false)
        );
    }

    #[test]
    fn test_nemotron_request_serialization_medium_reasoning() {
        let client = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            test_api_key(),
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_reasoning_effort(Some("medium".to_string()));

        let messages = vec![ChatMessage::user("Hi")];
        let req = client.build_request_body(&messages, None, false).unwrap();
        let json = serde_json::to_value(&req).unwrap();

        assert_eq!(json["reasoning_effort"], "medium");
        assert_eq!(
            json["chat_template_kwargs"]["enable_thinking"],
            serde_json::Value::Bool(true)
        );
    }

    #[test]
    fn test_nemotron_request_serialization_high_reasoning() {
        let client = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            test_api_key(),
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_reasoning_effort(Some("high".to_string()));

        let messages = vec![ChatMessage::user("Hi")];
        let req = client.build_request_body(&messages, None, false).unwrap();
        let json = serde_json::to_value(&req).unwrap();

        assert_eq!(json["reasoning_effort"], "high");
        assert_eq!(
            json["chat_template_kwargs"]["enable_thinking"],
            serde_json::Value::Bool(true)
        );
    }

    #[test]
    fn test_nemotron_request_serialization_explicit_reasoning_budget_256() {
        let client = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            test_api_key(),
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_max_tokens(Some(1024))
        .with_reasoning_budget(Some(256));

        let messages = vec![ChatMessage::user("Hi")];
        let req = client.build_request_body(&messages, None, false).unwrap();
        let json = serde_json::to_value(&req).unwrap();

        assert_eq!(json["reasoning_budget"], 256);
        assert_eq!(json["max_tokens"], 1024);
    }

    #[test]
    fn test_nemotron_request_serialization_explicit_reasoning_budget_512() {
        let client = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            test_api_key(),
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_max_tokens(Some(1024))
        .with_reasoning_budget(Some(512));

        let messages = vec![ChatMessage::user("Hi")];
        let req = client.build_request_body(&messages, None, false).unwrap();
        let json = serde_json::to_value(&req).unwrap();

        assert_eq!(json["reasoning_budget"], 512);
        assert_eq!(json["max_tokens"], 1024);
    }

    #[test]
    fn test_nemotron_request_serialization_explicit_output_budget_512() {
        let client = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            test_api_key(),
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_max_tokens(Some(512));

        let messages = vec![ChatMessage::user("Hi")];
        let req = client.build_request_body(&messages, None, false).unwrap();
        let json = serde_json::to_value(&req).unwrap();

        assert_eq!(json["max_tokens"], 512);
    }

    #[test]
    fn test_nemotron_request_serialization_tool_calling_with_reasoning_enabled() {
        let client = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            test_api_key(),
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_reasoning_effort(Some("medium".to_string()));

        let tools = vec![ToolDefinition {
            tool_type: "function".to_string(),
            function: m31a_core::types::FunctionDefinition {
                name: "lookup_symbol".to_string(),
                description: "Lookup a symbol".to_string(),
                parameters: serde_json::json!({"type": "object"}),
            },
        }];

        let messages = vec![ChatMessage::user("Search")];
        let req = client
            .build_request_body(&messages, Some(&tools), false)
            .unwrap();
        let json = serde_json::to_value(&req).unwrap();

        assert_eq!(
            json["chat_template_kwargs"]["enable_thinking"],
            serde_json::Value::Bool(true)
        );
        assert_eq!(
            json["chat_template_kwargs"]["force_nonempty_content"],
            serde_json::Value::Bool(true)
        );
        assert!(json["tools"].is_array());
        assert_eq!(json["tools"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_nemotron_request_serialization_tool_choice_required() {
        let client = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            test_api_key(),
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_tool_choice(Some("required".to_string()));

        let tools = vec![ToolDefinition {
            tool_type: "function".to_string(),
            function: m31a_core::types::FunctionDefinition {
                name: "calc".to_string(),
                description: "Calculate".to_string(),
                parameters: serde_json::json!({}),
            },
        }];

        let messages = vec![ChatMessage::user("Calculate")];
        let req = client
            .build_request_body(&messages, Some(&tools), false)
            .unwrap();
        let json = serde_json::to_value(&req).unwrap();

        assert_eq!(json["tool_choice"], "required");
    }

    #[test]
    fn test_nemotron_request_serialization_invalid_effort_enum() {
        let messages = vec![ChatMessage::user("Hi")];

        // "low" is unsupported for Nemotron Ultra and must be rejected
        let client_low = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            test_api_key(),
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_reasoning_effort(Some("low".to_string()));
        let err_low = client_low
            .build_request_body(&messages, None, false)
            .unwrap_err();
        assert!(
            err_low
                .to_string()
                .contains("Invalid reasoning_effort 'low'")
        );

        // Arbitrary unknown effort enum must be rejected
        let client_bogus = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            test_api_key(),
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_reasoning_effort(Some("extreme".to_string()));
        let err_bogus = client_bogus
            .build_request_body(&messages, None, false)
            .unwrap_err();
        assert!(
            err_bogus
                .to_string()
                .contains("Invalid reasoning_effort 'extreme'")
        );
    }

    #[test]
    fn test_nemotron_request_serialization_invalid_numeric_values() {
        let messages = vec![ChatMessage::user("Hi")];
        let key = test_api_key();

        // reasoning_budget < -1
        let c1 = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            &key,
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_reasoning_budget(Some(-2));
        assert!(
            c1.build_request_body(&messages, None, false)
                .unwrap_err()
                .to_string()
                .contains("reasoning_budget")
        );

        // reasoning_budget > 32768
        let c2 = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            &key,
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_reasoning_budget(Some(35000));
        assert!(
            c2.build_request_body(&messages, None, false)
                .unwrap_err()
                .to_string()
                .contains("reasoning_budget")
        );

        // max_tokens < 1
        let c3 = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            &key,
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_max_tokens(Some(0));
        assert!(
            c3.build_request_body(&messages, None, false)
                .unwrap_err()
                .to_string()
                .contains("max_tokens")
        );

        // max_tokens > 32768
        let c4 = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            &key,
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_max_tokens(Some(40000));
        assert!(
            c4.build_request_body(&messages, None, false)
                .unwrap_err()
                .to_string()
                .contains("max_tokens")
        );

        // temperature < 0.0 or > 2.0
        let c5 = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            &key,
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_temperature(Some(-0.1));
        assert!(
            c5.build_request_body(&messages, None, false)
                .unwrap_err()
                .to_string()
                .contains("temperature")
        );
        let c6 = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            &key,
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_temperature(Some(2.5));
        assert!(
            c6.build_request_body(&messages, None, false)
                .unwrap_err()
                .to_string()
                .contains("temperature")
        );

        // top_p < 0.0 or > 1.0
        let c7 = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            &key,
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_top_p(Some(-0.1));
        assert!(
            c7.build_request_body(&messages, None, false)
                .unwrap_err()
                .to_string()
                .contains("top_p")
        );
        let c8 = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            &key,
            "nvidia/nemotron-3-ultra-550b-a55b",
        )
        .with_top_p(Some(1.5));
        assert!(
            c8.build_request_body(&messages, None, false)
                .unwrap_err()
                .to_string()
                .contains("top_p")
        );
    }

    #[test]
    fn test_nemotron_request_serialization_missing_optional_parameters() {
        let client = NvidiaNimClient::new(
            "https://integrate.api.nvidia.com/v1",
            test_api_key(),
            "nvidia/nemotron-3-ultra-550b-a55b",
        );

        let messages = vec![ChatMessage::user("Hi")];
        let req = client.build_request_body(&messages, None, false).unwrap();
        let json = serde_json::to_value(&req).unwrap();

        // Optional parameters not configured must not appear in serialized JSON
        assert!(json.get("reasoning_budget").is_none());
        assert!(json.get("reasoning_effort").is_none());
        assert!(json.get("tool_choice").is_none());
        assert!(json.get("tools").is_none());
        assert!(json.get("max_tokens").is_none());
    }
}
