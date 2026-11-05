//! Cloud Tasks Client for Remote Agent Execution
//!
//! gRPC-based client for communicating with remote execution environments,
//! enabling scalable agent fleets and distributed workloads.

use anyhow::{Context, Result};
use async_trait::async_trait;
use m31a_core::types::{TokenUsage, ToolCall, ToolDefinition};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use uuid::Uuid;

pub mod proto {
    #![allow(clippy::result_large_err)]
    tonic::include_proto!("m31a.cloud.tasks.v1");
}

use proto::{
    ExecuteTaskRequest, ExecuteTaskResponse, GetCapabilitiesRequest, GetCapabilitiesResponse,
    SpawnAgentRequest, SpawnAgentResponse, cloud_tasks_client::CloudTasksClient,
};

/// Authentication methods for cloud tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CloudAuth {
    ApiKey { key: String },
    WorkloadIdentity { audience: String },
    OAuth2 { token: String },
    None,
}

/// Configuration for cloud tasks client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudTasksConfig {
    pub endpoint: String,
    pub auth: CloudAuth,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
    pub ca_cert_path: Option<String>,
    pub timeout_seconds: u64,
    pub max_retries: u32,
}

impl CloudTasksConfig {
    pub fn validate(&self) -> Result<()> {
        if self.endpoint.trim().is_empty() {
            anyhow::bail!("Cloud Tasks endpoint must not be empty");
        }
        match &self.auth {
            CloudAuth::ApiKey { key } => {
                if key.trim().is_empty() {
                    anyhow::bail!("Cloud Tasks API key must not be empty");
                }
            }
            CloudAuth::OAuth2 { token } => {
                if token.trim().is_empty() {
                    anyhow::bail!("Cloud Tasks OAuth2 token must not be empty");
                }
            }
            CloudAuth::WorkloadIdentity { audience } => {
                if audience.trim().is_empty() {
                    anyhow::bail!("Cloud Tasks Workload Identity audience must not be empty");
                }
            }
            CloudAuth::None => {}
        }
        if let Some(cert) = &self.tls_cert_path
            && !std::path::Path::new(cert).exists()
        {
            anyhow::bail!("TLS certificate path does not exist: {}", cert);
        }
        if let Some(key) = &self.tls_key_path
            && !std::path::Path::new(key).exists()
        {
            anyhow::bail!("TLS key path does not exist: {}", key);
        }
        if let Some(ca) = &self.ca_cert_path
            && !std::path::Path::new(ca).exists()
        {
            anyhow::bail!("CA certificate path does not exist: {}", ca);
        }
        Ok(())
    }
}

impl Default for CloudTasksConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://cloud-tasks.m31a.dev:443".to_string(),
            auth: CloudAuth::None,
            tls_cert_path: None,
            tls_key_path: None,
            ca_cert_path: None,
            timeout_seconds: 300,
            max_retries: 3,
        }
    }
}

/// Cloud Tasks Client for remote execution
#[derive(Clone)]
pub struct CloudTasksClientWrapper {
    inner: CloudTasksClient<Channel>,
    config: CloudTasksConfig,
}

impl CloudTasksClientWrapper {
    /// Create a new cloud tasks client
    pub async fn new(config: CloudTasksConfig) -> Result<Self> {
        let mut endpoint = Endpoint::from_shared(config.endpoint.clone())?;

        // Configure TLS if certificates provided
        if let (Some(cert_path), Some(key_path)) = (&config.tls_cert_path, &config.tls_key_path) {
            let cert = std::fs::read(cert_path)?;
            let key = std::fs::read(key_path)?;
            let identity = tonic::transport::Identity::from_pem(cert, key);
            endpoint = endpoint.tls_config(ClientTlsConfig::new().identity(identity))?;
        }

        if let Some(ca_path) = &config.ca_cert_path {
            let ca = std::fs::read(ca_path)?;
            let ca_cert = tonic::transport::Certificate::from_pem(ca);
            endpoint = endpoint.tls_config(ClientTlsConfig::new().ca_certificate(ca_cert))?;
        }

        // Configure timeouts
        endpoint = endpoint
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .connect_timeout(std::time::Duration::from_secs(10))
            .tcp_keepalive(Some(std::time::Duration::from_secs(30)));

        let channel = endpoint
            .connect()
            .await
            .context("Failed to connect to cloud tasks service")?;

        let inner = CloudTasksClient::new(channel);

        Ok(Self { inner, config })
    }

    /// Execute a task remotely
    pub async fn execute_task(&self, task: TaskSpec) -> Result<TaskResult> {
        let request = ExecuteTaskRequest {
            task_spec: Some(task.to_proto()),
        };

        let response = self
            .retry_with_backoff(|mut client| {
                let req = request.clone();
                async move { client.execute_task(req).await }
            })
            .await?;

        Ok(TaskResult::from_proto(response))
    }

    /// Spawn a remote agent
    pub async fn spawn_agent(&self, req: SpawnAgentRequest) -> Result<AgentHandleWrapper> {
        let request = SpawnAgentRequest {
            session_id: req.session_id,
            model: req.model,
            config: req.config,
            capabilities: req.capabilities,
            working_dir: req.working_dir,
        };

        let response = self
            .retry_with_backoff(|mut client| {
                let req = request.clone();
                async move { client.spawn_agent(req).await }
            })
            .await?;

        Ok(AgentHandleWrapper::from_proto(response))
    }

    /// Get execution capabilities of the remote environment
    pub async fn get_capabilities(&self) -> Result<ExecutionCapabilitiesWrapper> {
        let request = GetCapabilitiesRequest {};

        let response = self
            .retry_with_backoff(|mut client| {
                let req = request;
                async move { client.get_capabilities(req).await }
            })
            .await?;

        Ok(ExecutionCapabilitiesWrapper::from_proto(response))
    }

    /// Send input to a running agent
    pub async fn send_agent_input(&self, agent_id: &str, input: String) -> Result<()> {
        let request = proto::SendAgentInputRequest {
            agent_id: agent_id.to_string(),
            input,
        };

        let _ = self
            .retry_with_backoff(|mut client| {
                let req = request.clone();
                async move { client.send_agent_input(req).await }
            })
            .await?;

        Ok(())
    }

    /// Wait for agent completion
    pub async fn wait_agent(&self, agent_id: &str) -> Result<TaskResult> {
        let request = proto::WaitAgentRequest {
            agent_id: agent_id.to_string(),
        };

        let response = self
            .retry_with_backoff(|mut client| {
                let req = request.clone();
                async move { client.wait_agent(req).await }
            })
            .await?;

        Ok(TaskResult::from_proto(response))
    }

    /// Close/terminate an agent
    pub async fn close_agent(&self, agent_id: &str) -> Result<()> {
        let request = proto::CloseAgentRequest {
            agent_id: agent_id.to_string(),
        };

        let _ = self
            .retry_with_backoff(|mut client| {
                let req = request.clone();
                async move { client.close_agent(req).await }
            })
            .await?;

        Ok(())
    }

    /// List active agents
    pub async fn list_agents(&self) -> Result<Vec<AgentHandleWrapper>> {
        let request = proto::ListAgentsRequest {};

        let response = self
            .retry_with_backoff(|mut client| {
                let req = request;
                async move { client.list_agents(req).await }
            })
            .await?;

        Ok(response
            .agents
            .into_iter()
            .map(AgentHandleWrapper::from_proto)
            .collect())
    }

    /// Retry with exponential backoff
    async fn retry_with_backoff<F, Fut, T>(&self, mut f: F) -> Result<T>
    where
        F: FnMut(CloudTasksClient<Channel>) -> Fut,
        Fut: std::future::Future<Output = Result<tonic::Response<T>, tonic::Status>>,
    {
        let mut last_error = None;
        for attempt in 0..=self.config.max_retries {
            let client = self.inner.clone();
            match f(client).await {
                Ok(response) => return Ok(response.into_inner()),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.config.max_retries {
                        let delay = std::time::Duration::from_millis(100 * 2_u64.pow(attempt));
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        Err(last_error.unwrap().into())
    }
}

/// Task specification for remote execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub id: String,
    pub prompt: String,
    pub model: String,
    pub tools: Vec<ToolDefinition>,
    pub working_dir: String,
    pub env_vars: HashMap<String, String>,
    pub timeout_seconds: u64,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

impl TaskSpec {
    pub fn to_proto(&self) -> proto::TaskSpec {
        proto::TaskSpec {
            id: self.id.clone(),
            prompt: self.prompt.clone(),
            model: self.model.clone(),
            tools: self.tools.iter().map(|t| t.into()).collect(),
            working_dir: self.working_dir.clone(),
            env_vars: self.env_vars.clone(),
            timeout_seconds: self.timeout_seconds as u32,
            max_tokens: self.max_tokens.unwrap_or(0),
            temperature: self.temperature.unwrap_or(0.7),
        }
    }
}

impl From<&ToolDefinition> for proto::ToolDefinition {
    fn from(t: &ToolDefinition) -> Self {
        proto::ToolDefinition {
            name: t.function.name.clone(),
            description: t.function.description.clone(),
            parameters_json: t.function.parameters.to_string(),
        }
    }
}

impl From<proto::ToolCall> for ToolCall {
    fn from(p: proto::ToolCall) -> Self {
        ToolCall {
            id: p.id,
            call_type: "function".to_string(),
            function: m31a_core::types::FunctionCall {
                name: p.name,
                arguments: p.arguments_json,
            },
        }
    }
}

impl From<proto::TokenUsage> for TokenUsage {
    fn from(p: proto::TokenUsage) -> Self {
        TokenUsage {
            prompt_tokens: p.prompt_tokens,
            completion_tokens: p.completion_tokens,
            reasoning_tokens: p.reasoning_tokens,
            total_tokens: p.total_tokens,
        }
    }
}

/// Result of a task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub status: TaskStatusEnum,
    pub output: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: TokenUsage,
    pub error: Option<String>,
    pub duration_ms: u64,
}

impl TaskResult {
    fn from_proto(proto: ExecuteTaskResponse) -> Self {
        Self {
            task_id: proto.task_id,
            status: TaskStatusEnum::from(proto.status),
            output: proto.output,
            tool_calls: proto.tool_calls.into_iter().map(Into::into).collect(),
            usage: proto.usage.map(Into::into).unwrap_or_default(),
            error: if proto.error.is_empty() {
                None
            } else {
                Some(proto.error)
            },
            duration_ms: proto.duration_ms,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatusEnum {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl From<i32> for TaskStatusEnum {
    fn from(status: i32) -> Self {
        match status {
            0 => Self::Pending,
            1 => Self::Running,
            2 => Self::Completed,
            3 => Self::Failed,
            4 => Self::Cancelled,
            _ => Self::Failed,
        }
    }
}

/// Remote agent handle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHandleWrapper {
    pub agent_id: String,
    pub session_id: String,
    pub status: AgentStatusEnum,
    pub capabilities: Vec<String>,
    pub created_at: String,
}

impl AgentHandleWrapper {
    fn from_proto(proto: SpawnAgentResponse) -> Self {
        Self {
            agent_id: proto.agent_id,
            session_id: proto.session_id,
            status: AgentStatusEnum::from(proto.status),
            capabilities: proto.capabilities,
            created_at: proto.created_at,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatusEnum {
    Starting,
    Ready,
    Busy,
    Completed,
    Failed,
    Terminated,
}

impl From<i32> for AgentStatusEnum {
    fn from(status: i32) -> Self {
        match status {
            0 => Self::Starting,
            1 => Self::Ready,
            2 => Self::Busy,
            3 => Self::Completed,
            4 => Self::Failed,
            5 => Self::Terminated,
            _ => Self::Failed,
        }
    }
}

/// Execution capabilities of remote environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionCapabilitiesWrapper {
    pub supports_file_ops: bool,
    pub supports_process_exec: bool,
    pub supports_network: bool,
    pub supports_gpu: bool,
    pub max_concurrent_agents: u32,
    pub max_memory_mb: u32,
    pub max_cpu_cores: u32,
    pub available_models: Vec<String>,
    pub supported_tools: Vec<String>,
}

impl ExecutionCapabilitiesWrapper {
    fn from_proto(proto: GetCapabilitiesResponse) -> Self {
        Self {
            supports_file_ops: proto.supports_file_ops,
            supports_process_exec: proto.supports_process_exec,
            supports_network: proto.supports_network,
            supports_gpu: proto.supports_gpu,
            max_concurrent_agents: proto.max_concurrent_agents,
            max_memory_mb: proto.max_memory_mb,
            max_cpu_cores: proto.max_cpu_cores,
            available_models: proto.available_models,
            supported_tools: proto.supported_tools,
        }
    }
}

/// Trait for cloud task execution (allows mocking)
#[async_trait]
pub trait CloudTaskExecutor: Send + Sync {
    async fn execute_task(&self, task: TaskSpec) -> Result<TaskResult>;
    async fn spawn_agent(&self, req: SpawnAgentRequest) -> Result<AgentHandleWrapper>;
    async fn get_capabilities(&self) -> Result<ExecutionCapabilitiesWrapper>;
    async fn send_agent_input(&self, agent_id: &str, input: String) -> Result<()>;
    async fn wait_agent(&self, agent_id: &str) -> Result<TaskResult>;
    async fn close_agent(&self, agent_id: &str) -> Result<()>;
    async fn list_agents(&self) -> Result<Vec<AgentHandleWrapper>>;
}

#[async_trait]
impl CloudTaskExecutor for CloudTasksClientWrapper {
    async fn execute_task(&self, task: TaskSpec) -> Result<TaskResult> {
        self.execute_task(task).await
    }

    async fn spawn_agent(&self, req: SpawnAgentRequest) -> Result<AgentHandleWrapper> {
        self.spawn_agent(req).await
    }

    async fn get_capabilities(&self) -> Result<ExecutionCapabilitiesWrapper> {
        self.get_capabilities().await
    }

    async fn send_agent_input(&self, agent_id: &str, input: String) -> Result<()> {
        self.send_agent_input(agent_id, input).await
    }

    async fn wait_agent(&self, agent_id: &str) -> Result<TaskResult> {
        self.wait_agent(agent_id).await
    }

    async fn close_agent(&self, agent_id: &str) -> Result<()> {
        self.close_agent(agent_id).await
    }

    async fn list_agents(&self) -> Result<Vec<AgentHandleWrapper>> {
        self.list_agents().await
    }
}

/// Local execution and fallback client for offline/containerized tasks
#[derive(Clone)]
pub struct LocalFallbackCloudTasksClient {
    running_agents: std::sync::Arc<tokio::sync::RwLock<HashMap<String, AgentHandleWrapper>>>,
    agent_inputs: std::sync::Arc<tokio::sync::RwLock<HashMap<String, Vec<String>>>>,
}

impl Default for LocalFallbackCloudTasksClient {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalFallbackCloudTasksClient {
    pub fn new() -> Self {
        Self {
            running_agents: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            agent_inputs: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl CloudTaskExecutor for LocalFallbackCloudTasksClient {
    async fn execute_task(&self, task: TaskSpec) -> Result<TaskResult> {
        let start = std::time::Instant::now();

        // Execute prompt command locally if it looks like a shell command
        let (output, status, error) = if task.prompt.starts_with('!')
            || task.prompt.contains("cargo")
            || task.prompt.contains("ls")
            || task.prompt.contains("echo")
        {
            let cmd_str = task.prompt.trim_start_matches('!').trim();
            let mut parts = cmd_str.split_whitespace();
            if let Some(cmd) = parts.next() {
                let args: Vec<&str> = parts.collect();
                let cwd = if !task.working_dir.is_empty()
                    && std::path::Path::new(&task.working_dir).exists()
                {
                    std::path::PathBuf::from(&task.working_dir)
                } else {
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
                };
                let res = tokio::process::Command::new(cmd)
                    .args(&args)
                    .current_dir(cwd)
                    .output()
                    .await;
                match res {
                    Ok(o) => {
                        let stdout = String::from_utf8_lossy(&o.stdout);
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        if o.status.success() {
                            let text = if stdout.is_empty() {
                                "Command succeeded with no output".to_string()
                            } else {
                                stdout.to_string()
                            };
                            (text, TaskStatusEnum::Completed, None)
                        } else {
                            let err_msg = format!(
                                "Exit {}: {}{}",
                                o.status.code().unwrap_or(-1),
                                stdout,
                                stderr
                            );
                            (String::new(), TaskStatusEnum::Failed, Some(err_msg))
                        }
                    }
                    Err(e) => (
                        String::new(),
                        TaskStatusEnum::Failed,
                        Some(format!("Execution spawn error: {e}")),
                    ),
                }
            } else {
                (
                    String::new(),
                    TaskStatusEnum::Failed,
                    Some("Empty command provided".to_string()),
                )
            }
        } else {
            (
                String::new(),
                TaskStatusEnum::Failed,
                Some(format!(
                    "LocalFallbackCloudTasksClient only supports direct local commands (e.g. prefix with '!'). Task objective '{}' requires a connected remote Cloud Tasks endpoint.",
                    task.prompt
                )),
            )
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(TaskResult {
            task_id: task.id,
            status,
            output,
            tool_calls: vec![],
            usage: TokenUsage::default(),
            error,
            duration_ms,
        })
    }

    async fn spawn_agent(&self, req: SpawnAgentRequest) -> Result<AgentHandleWrapper> {
        let handle = AgentHandleWrapper {
            agent_id: Uuid::new_v4().to_string(),
            session_id: req.session_id,
            status: AgentStatusEnum::Ready,
            capabilities: if req.capabilities.is_empty() {
                vec![
                    "fs".to_string(),
                    "terminal".to_string(),
                    "tools".to_string(),
                ]
            } else {
                req.capabilities
            },
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        self.running_agents
            .write()
            .await
            .insert(handle.agent_id.clone(), handle.clone());
        Ok(handle)
    }

    async fn get_capabilities(&self) -> Result<ExecutionCapabilitiesWrapper> {
        let num_cpus = std::thread::available_parallelism()
            .map(|p| p.get() as u32)
            .unwrap_or(4);
        Ok(ExecutionCapabilitiesWrapper {
            supports_file_ops: true,
            supports_process_exec: true,
            supports_network: true,
            supports_gpu: false,
            max_concurrent_agents: 16,
            max_memory_mb: 16384,
            max_cpu_cores: num_cpus,
            available_models: vec![
                "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
                "deepseek-ai/deepseek-r1".to_string(),
            ],
            supported_tools: vec![
                "exec_command".to_string(),
                "read_file".to_string(),
                "write_file".to_string(),
                "apply_patch".to_string(),
            ],
        })
    }

    async fn send_agent_input(&self, agent_id: &str, input: String) -> Result<()> {
        let agents = self.running_agents.read().await;
        if !agents.contains_key(agent_id) {
            anyhow::bail!("Agent '{}' not found", agent_id);
        }
        self.agent_inputs
            .write()
            .await
            .entry(agent_id.to_string())
            .or_default()
            .push(input);
        Ok(())
    }

    async fn wait_agent(&self, agent_id: &str) -> Result<TaskResult> {
        let agents = self.running_agents.read().await;
        let agent = agents
            .get(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent '{}' not found", agent_id))?
            .clone();
        drop(agents);

        let inputs = self.agent_inputs.write().await.remove(agent_id);
        let start = std::time::Instant::now();

        let (output, status, error) = if let Some(inputs) = inputs
            && !inputs.is_empty()
        {
            let mut combined_output = String::new();
            let mut final_status = TaskStatusEnum::Completed;
            let mut first_error = None;

            for raw_input in inputs {
                let cmd_str = raw_input.trim_start_matches('!').trim();
                let res = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(cmd_str)
                    .output()
                    .await;

                match res {
                    Ok(o) => {
                        let stdout = String::from_utf8_lossy(&o.stdout);
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        if o.status.success() {
                            if !stdout.is_empty() {
                                combined_output.push_str(&stdout);
                            }
                        } else {
                            final_status = TaskStatusEnum::Failed;
                            let err_msg = format!(
                                "Exit {}: {}{}",
                                o.status.code().unwrap_or(-1),
                                stdout,
                                stderr
                            );
                            first_error = Some(err_msg);
                            break;
                        }
                    }
                    Err(e) => {
                        final_status = TaskStatusEnum::Failed;
                        first_error = Some(format!("Local command spawn error: {e}"));
                        break;
                    }
                }
            }

            if combined_output.is_empty() && final_status == TaskStatusEnum::Completed {
                combined_output = "Command succeeded with no output".to_string();
            }

            (combined_output, final_status, first_error)
        } else {
            (
                String::new(),
                TaskStatusEnum::Failed,
                Some(
                    "No input was provided to the local fallback agent before wait_agent"
                        .to_string(),
                ),
            )
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(TaskResult {
            task_id: agent.agent_id,
            status,
            output,
            tool_calls: vec![],
            usage: TokenUsage::default(),
            error,
            duration_ms,
        })
    }

    async fn close_agent(&self, agent_id: &str) -> Result<()> {
        self.running_agents.write().await.remove(agent_id);
        self.agent_inputs.write().await.remove(agent_id);
        Ok(())
    }

    async fn list_agents(&self) -> Result<Vec<AgentHandleWrapper>> {
        let agents = self.running_agents.read().await;
        Ok(agents.values().cloned().collect())
    }
}

/// Unified dispatcher that attempts remote gRPC execution, falling back to local runner
#[derive(Clone)]
pub struct CloudTaskDispatcher {
    remote_client: Option<CloudTasksClientWrapper>,
    local_fallback: LocalFallbackCloudTasksClient,
}

impl CloudTaskDispatcher {
    pub fn new(remote_client: Option<CloudTasksClientWrapper>) -> Self {
        Self {
            remote_client,
            local_fallback: LocalFallbackCloudTasksClient::new(),
        }
    }

    pub fn local_only() -> Self {
        Self::new(None)
    }

    pub async fn execute_task(&self, task: TaskSpec) -> Result<TaskResult> {
        if let Some(ref client) = self.remote_client {
            match client.execute_task(task.clone()).await {
                Ok(res) => return Ok(res),
                Err(e) => {
                    tracing::warn!(
                        "Remote cloud task execution failed ({}); falling back to local runner",
                        e
                    );
                }
            }
        }
        self.local_fallback.execute_task(task).await
    }

    pub async fn spawn_agent(&self, req: SpawnAgentRequest) -> Result<AgentHandleWrapper> {
        if let Some(ref client) = self.remote_client {
            match client.spawn_agent(req.clone()).await {
                Ok(res) => return Ok(res),
                Err(e) => {
                    tracing::warn!(
                        "Remote agent spawn failed ({}); falling back to local runner",
                        e
                    );
                }
            }
        }
        self.local_fallback.spawn_agent(req).await
    }

    pub async fn get_capabilities(&self) -> Result<ExecutionCapabilitiesWrapper> {
        if let Some(ref client) = self.remote_client
            && let Ok(caps) = client.get_capabilities().await
        {
            return Ok(caps);
        }
        self.local_fallback.get_capabilities().await
    }
}

pub type MockCloudTasksClient = LocalFallbackCloudTasksClient;

impl SpawnAgentRequest {
    pub fn new(
        session_id: String,
        model: String,
        config: Option<String>,
        capabilities: Vec<String>,
        working_dir: String,
    ) -> Self {
        Self {
            session_id,
            model,
            config: config.unwrap_or_default(),
            capabilities,
            working_dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use m31a_core::types::FunctionDefinition;

    #[tokio::test]
    async fn test_mock_cloud_tasks_executor_lifecycle() {
        let client = MockCloudTasksClient::new();

        // Capabilities
        let caps = client.get_capabilities().await.expect("capabilities");
        assert!(caps.supports_file_ops);
        assert!(caps.supports_process_exec);
        assert!(caps.max_concurrent_agents >= 10);

        // Spawn agent
        let spawn_req = SpawnAgentRequest::new(
            "session-alpha".to_string(),
            "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
            None,
            vec!["fs".to_string(), "terminal".to_string()],
            "/workspace".to_string(),
        );
        let agent = client.spawn_agent(spawn_req).await.expect("spawn agent");
        assert_eq!(agent.session_id, "session-alpha");
        assert_eq!(agent.status, AgentStatusEnum::Ready);

        // Send input
        client
            .send_agent_input(&agent.agent_id, "echo build target".to_string())
            .await
            .expect("send input");

        // Wait for agent
        let wait_res = client
            .wait_agent(&agent.agent_id)
            .await
            .expect("wait agent");
        assert_eq!(wait_res.status, TaskStatusEnum::Completed);

        // Verify genuine failure propagation: non-zero exit MUST fail honestly
        client
            .send_agent_input(&agent.agent_id, "exit 7".to_string())
            .await
            .expect("send input");
        let fail_res = client
            .wait_agent(&agent.agent_id)
            .await
            .expect("wait agent");
        assert_eq!(fail_res.status, TaskStatusEnum::Failed);
        assert!(fail_res.error.unwrap().contains("7"));

        // Execute discrete task
        let task = TaskSpec {
            id: "task-001".to_string(),
            prompt: "!echo 'Check compiler status'".to_string(),
            model: "deepseek-ai/deepseek-r1".to_string(),
            tools: vec![ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "exec_command".to_string(),
                    description: "Execute terminal command".to_string(),
                    parameters: serde_json::json!({}),
                },
            }],
            working_dir: ".".to_string(),
            env_vars: HashMap::new(),
            timeout_seconds: 60,
            max_tokens: Some(4096),
            temperature: Some(0.2),
        };

        let proto_spec = task.to_proto();
        assert_eq!(proto_spec.id, "task-001");
        assert_eq!(proto_spec.timeout_seconds, 60);
        assert_eq!(proto_spec.tools.len(), 1);
        assert_eq!(proto_spec.tools[0].name, "exec_command");

        let task_res = client.execute_task(task).await.expect("execute task");
        assert_eq!(task_res.task_id, "task-001");
        assert_eq!(task_res.status, TaskStatusEnum::Completed);
        assert!(task_res.output.contains("Check compiler status"));

        // Reject non-shell prompt without remote endpoint
        let non_shell = TaskSpec {
            id: "task-002".to_string(),
            prompt: "Please write a summary of this repo".to_string(),
            model: "deepseek-ai/deepseek-r1".to_string(),
            tools: vec![],
            working_dir: ".".to_string(),
            env_vars: HashMap::new(),
            timeout_seconds: 60,
            max_tokens: None,
            temperature: None,
        };
        let non_shell_res = client
            .execute_task(non_shell)
            .await
            .expect("execute non-shell");
        assert_eq!(non_shell_res.status, TaskStatusEnum::Failed);
        assert!(
            non_shell_res
                .error
                .unwrap()
                .contains("requires a connected remote Cloud Tasks endpoint")
        );
    }

    #[test]
    fn test_cloud_tasks_config_and_auth_serialization() {
        let config = CloudTasksConfig::default();
        assert_eq!(config.timeout_seconds, 300);
        assert_eq!(config.max_retries, 3);

        let json = serde_json::to_string(&config).expect("serialize config");
        let parsed: CloudTasksConfig = serde_json::from_str(&json).expect("deserialize config");
        assert_eq!(parsed.endpoint, config.endpoint);
    }
}
