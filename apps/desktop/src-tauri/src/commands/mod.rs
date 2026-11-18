//! Strongly-typed Tauri IPC command handlers for M31A Desktop.
//!
//! Exposes authoritative runtime operations across lifecycle, workspace,
//! models, sessions, turns, and approvals.

use crate::security::{validate_and_canonicalize_workspace_path, verify_workspace_trust};
use crate::state::DesktopState;
use m31a_core::prompts::CollaborationMode;
use m31a_core::types::SessionMetadata;
use m31a_elicitation::ApprovalDecision;
use m31a_provider::models::ModelInfo;
use m31a_runtime::context::TurnRequest;
use m31a_runtime::lifecycle::ShutdownReport;
use m31a_trust::TrustDecision;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

// =============================================================================
// DTOs
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatusDto {
    pub state: String,
    pub active_model: String,
    pub active_workspace: String,
    pub is_operational: bool,
    pub is_terminal: bool,
    pub active_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentInfoDto {
    pub app_version: String,
    pub target_platform: String,
    pub target_arch: String,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfoDto {
    pub path: String,
    pub is_trusted: bool,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfoDto {
    pub id: String,
    pub name: String,
    pub originator: String,
    pub context_window: usize,
    pub supports_tools: bool,
    pub supports_thinking: bool,
    pub supports_vision: bool,
}

impl From<ModelInfo> for ModelInfoDto {
    fn from(m: ModelInfo) -> Self {
        Self {
            id: m.slug,
            name: m.display_name,
            originator: m.originator,
            context_window: m.context_window as usize,
            supports_tools: m.supports_tools,
            supports_thinking: m.supports_thinking,
            supports_vision: m.supports_vision,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummaryDto {
    pub id: String,
    pub title: String,
    pub model: String,
    pub cwd: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnSummaryDto {
    pub id: String,
    pub thread_id: String,
    pub turn_index: usize,
    pub status: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessageDto {
    pub id: String,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDetailsDto {
    pub id: String,
    pub title: String,
    pub model: String,
    pub cwd: String,
    pub workspace_is_trusted: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub turns: Vec<TurnSummaryDto>,
    pub messages: Vec<SessionMessageDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitTurnPayload {
    pub thread_id: Option<String>,
    pub prompt: String,
    pub collaboration_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnHandleDto {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingInteractionsDto {
    pub approvals: Vec<PendingApprovalDto>,
    pub questions: Vec<PendingQuestionDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingApprovalDto {
    pub request_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub command: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingQuestionDto {
    pub question_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub question: String,
    pub options: Vec<String>,
    pub multi_select: bool,
}

// =============================================================================
// Lifecycle Commands
// =============================================================================

#[tauri::command]
pub async fn get_runtime_status(
    state: State<'_, DesktopState>,
) -> Result<RuntimeStatusDto, String> {
    let runtime = state.runtime();
    let r_state = runtime.state();
    let active_model = runtime.active_model();
    let active_workspace = state.active_workspace().display().to_string();
    let active_session_id = state.active_session_id();

    Ok(RuntimeStatusDto {
        state: format!("{:?}", r_state),
        active_model,
        active_workspace,
        is_operational: r_state.is_operational(),
        is_terminal: r_state.is_terminal(),
        active_session_id,
    })
}

#[tauri::command]
pub async fn get_environment_info(
    state: State<'_, DesktopState>,
) -> Result<EnvironmentInfoDto, String> {
    let runtime = state.runtime();
    let features: Vec<String> = runtime
        .features_registry()
        .active_features()
        .into_iter()
        .map(|k| k.as_str().to_string())
        .collect();

    Ok(EnvironmentInfoDto {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        target_platform: std::env::consts::OS.to_string(),
        target_arch: std::env::consts::ARCH.to_string(),
        features,
    })
}

#[tauri::command]
pub async fn shutdown_runtime(state: State<'_, DesktopState>) -> Result<ShutdownReport, String> {
    state.mark_shutting_down();
    state.runtime().shutdown().await.map_err(|e| e.to_string())
}

// =============================================================================
// Workspace Commands
// =============================================================================

#[tauri::command]
pub async fn get_active_workspace(
    state: State<'_, DesktopState>,
) -> Result<WorkspaceInfoDto, String> {
    let ws = state.active_workspace();
    let trust_store = state.runtime().trust_store();
    let is_trusted = trust_store
        .lock()
        .map(|store| {
            matches!(
                store.evaluate(&ws),
                TrustDecision::Trusted | TrustDecision::SessionOnly
            )
        })
        .unwrap_or(false);

    Ok(WorkspaceInfoDto {
        path: ws.display().to_string(),
        is_trusted,
        exists: ws.exists(),
    })
}

#[tauri::command]
pub async fn set_workspace(
    state: State<'_, DesktopState>,
    path: String,
) -> Result<WorkspaceInfoDto, String> {
    let canonical = validate_and_canonicalize_workspace_path(&path).map_err(|e| e.to_string())?;

    // Selecting a workspace must not authorize it. Untrusted paths may be
    // selected, but execution remains blocked until native confirmation.
    let trust_store = state.runtime().trust_store();
    let is_trusted =
        verify_workspace_trust(&trust_store, &canonical, false).map_err(|e| e.to_string())?;

    state.set_active_workspace(canonical.clone());

    Ok(WorkspaceInfoDto {
        path: canonical.display().to_string(),
        is_trusted,
        exists: true,
    })
}

#[tauri::command]
pub async fn authorize_workspace(
    app: AppHandle,
    state: State<'_, DesktopState>,
    path: String,
) -> Result<WorkspaceInfoDto, String> {
    let canonical = validate_and_canonicalize_workspace_path(&path).map_err(|e| e.to_string())?;
    let trust_store = state.runtime().trust_store();

    // Already-trusted workspaces do not need another prompt. Denied workspaces
    // fail closed before the user is shown any authorization affordance.
    if verify_workspace_trust(&trust_store, &canonical, false)
        .map_err(|e| e.to_string())?
    {
        return Ok(WorkspaceInfoDto {
            path: canonical.display().to_string(),
            is_trusted: true,
            exists: true,
        });
    }

    let confirmed = app
        .dialog()
        .message(format!(
            "Allow M31A to execute tools and modify files in this workspace for this session?\n\n{}",
            canonical.display()
        ))
        .title("Authorize M31A Workspace")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Trust for This Session".to_string(),
            "Cancel".to_string(),
        ))
        .blocking_show();

    if !confirmed {
        return Err("Workspace authorization was cancelled".to_string());
    }

    // Re-evaluate after the native prompt. In particular, a concurrent or
    // persistent Denied decision must never be overridden by this grant.
    verify_workspace_trust(&trust_store, &canonical, true).map_err(|e| e.to_string())?;

    Ok(WorkspaceInfoDto {
        path: canonical.display().to_string(),
        is_trusted: true,
        exists: true,
    })
}

// =============================================================================
// Model Commands
// =============================================================================

#[tauri::command]
pub async fn list_available_models(
    state: State<'_, DesktopState>,
) -> Result<Vec<ModelInfoDto>, String> {
    let models = state.runtime().available_models();
    Ok(models.into_iter().map(ModelInfoDto::from).collect())
}

#[tauri::command]
pub async fn get_active_model(state: State<'_, DesktopState>) -> Result<String, String> {
    Ok(state.runtime().active_model())
}

#[tauri::command]
pub async fn activate_model(
    state: State<'_, DesktopState>,
    model: String,
) -> Result<String, String> {
    let clean_model = model.trim();
    if clean_model.is_empty() {
        return Err("Model slug cannot be empty".to_string());
    }

    state
        .runtime()
        .switch_model(clean_model)
        .map_err(|e| e.to_string())?;

    Ok(clean_model.to_string())
}

// =============================================================================
// Session Commands
// =============================================================================

#[tauri::command]
pub async fn list_sessions(
    state: State<'_, DesktopState>,
) -> Result<Vec<SessionSummaryDto>, String> {
    let storage = state.runtime().storage();
    let threads = storage.list_threads(100).map_err(|e| e.to_string())?;
    let active_id = state.active_session_id();

    let dtos = threads
        .into_iter()
        .map(|t| SessionSummaryDto {
            id: t.id.clone(),
            title: t.title,
            model: t.model,
            cwd: t.cwd,
            created_at: t.created_at,
            updated_at: t.updated_at,
            is_active: Some(&t.id) == active_id.as_ref(),
        })
        .collect();

    Ok(dtos)
}

#[tauri::command]
pub async fn create_session(
    state: State<'_, DesktopState>,
    title: Option<String>,
) -> Result<SessionSummaryDto, String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let session_title = title.unwrap_or_else(|| "New Session".to_string());
    let active_model = state.runtime().active_model();
    let cwd = state.active_workspace().display().to_string();
    let now = chrono::Utc::now();

    let meta = SessionMetadata {
        id: session_id.clone(),
        title: session_title.clone(),
        model: active_model.clone(),
        cwd: cwd.clone(),
        created_at: now,
        updated_at: now,
        extra: HashMap::new(),
    };

    let storage = state.runtime().storage();
    storage
        .save_thread(&meta, "", "standard", "standard")
        .map_err(|e| e.to_string())?;

    state.set_active_session_id(Some(session_id.clone()));

    Ok(SessionSummaryDto {
        id: session_id,
        title: session_title,
        model: active_model,
        cwd,
        created_at: now.timestamp(),
        updated_at: now.timestamp(),
        is_active: true,
    })
}

#[tauri::command]
pub async fn load_session(
    state: State<'_, DesktopState>,
    session_id: String,
) -> Result<SessionDetailsDto, String> {
    let storage = state.runtime().storage();
    let thread = storage
        .get_thread(&session_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Session '{session_id}' not found"))?;

    let turns = storage
        .list_turns(&session_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|t| TurnSummaryDto {
            id: t.id,
            thread_id: t.thread_id,
            turn_index: t.turn_index,
            status: t.status,
            created_at: t.created_at,
        })
        .collect();

    let session_workspace = validate_and_canonicalize_workspace_path(&thread.cwd)
        .map_err(|error| format!("Session workspace is unavailable: {error}"))?;
    let trust_store = state.runtime().trust_store();
    let workspace_is_trusted = verify_workspace_trust(&trust_store, &session_workspace, false)
        .map_err(|error| error.to_string())?;

    let messages = storage
        .list_items(&session_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter_map(|(item_type, content, id)| {
            let role = match item_type.as_str() {
                "user" | "user_message" | "user_queued" => "user",
                "assistant" | "assistant_message" | "agent_response" => "assistant",
                _ => return None,
            };
            Some(SessionMessageDto {
                id,
                role: role.to_string(),
                content,
            })
        })
        .collect();

    state.set_active_workspace(session_workspace);
    state.set_active_session_id(Some(session_id.clone()));

    Ok(SessionDetailsDto {
        id: thread.id,
        title: thread.title,
        model: thread.model,
        cwd: thread.cwd,
        workspace_is_trusted,
        created_at: thread.created_at,
        updated_at: thread.updated_at,
        turns,
        messages,
    })
}

// =============================================================================
// Turn Commands
// =============================================================================

#[tauri::command]
pub async fn submit_turn(
    state: State<'_, DesktopState>,
    payload: SubmitTurnPayload,
) -> Result<TurnHandleDto, String> {
    let trimmed_prompt = payload.prompt.trim();
    if trimmed_prompt.is_empty() {
        return Err("Prompt cannot be empty".to_string());
    }

    // Resolve or automatically create the active session, then bind execution
    // to the session's persisted workspace rather than trusting a renderer path.
    let thread_id = match payload.thread_id.or_else(|| state.active_session_id()) {
        Some(id) => id,
        None => create_session(state.clone(), None).await?.id,
    };
    let storage = state.runtime().storage();
    let thread = storage
        .get_thread(&thread_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Session '{thread_id}' does not exist"))?;
    let session_workspace = validate_and_canonicalize_workspace_path(&thread.cwd)
        .map_err(|error| format!("Session workspace is unavailable: {error}"))?;
    let active_workspace = validate_and_canonicalize_workspace_path(
        &state.active_workspace().display().to_string(),
    )
    .map_err(|error| error.to_string())?;
    if session_workspace != active_workspace {
        return Err(format!(
            "Session belongs to '{}', but the active workspace is '{}'. Select the session workspace before submitting a turn.",
            session_workspace.display(),
            active_workspace.display()
        ));
    }

    let trust_store = state.runtime().trust_store();
    if !verify_workspace_trust(&trust_store, &session_workspace, false)
        .map_err(|error| error.to_string())?
    {
        return Err("Workspace requires explicit native confirmation before execution".to_string());
    }

    let collab_mode = match payload.collaboration_mode.as_deref() {
        Some("plan") => CollaborationMode::Plan,
        Some("review") => CollaborationMode::Review,
        _ => CollaborationMode::Default,
    };

    // Persist the user's prompt before starting the runtime so the runtime can
    // rehydrate prior conversation history consistently for this session.
    storage
        .save_item(
            &uuid::Uuid::new_v4().to_string(),
            &thread_id,
            None,
            "user",
            trimmed_prompt,
        )
        .map_err(|error| error.to_string())?;

    let request = TurnRequest {
        thread_id: thread_id.clone(),
        prompt: trimmed_prompt.to_string(),
        workspace_root: Some(session_workspace),
        allow_untrusted_workspace: false,
        images: Vec::new(),
        collaboration_mode: collab_mode,
        ephemeral: false,
        self_heal: false,
        max_heal_iterations: 0,
        max_steps: 50,
    };

    let (turn_id, mut out_rx) = state
        .runtime()
        .execute_turn_with_id(request)
        .await
        .map_err(|error| error.to_string())?;

    let thread_id_ret = thread_id.clone();
    let turn_id_ret = turn_id.clone();

    // Spawn a lightweight task to consume the turn receiver channel so it flows freely
    tauri::async_runtime::spawn(async move {
        while let Some(_event) = out_rx.recv().await {
            // Events are broadcast to subscribers by runtime
        }
    });

    Ok(TurnHandleDto {
        thread_id: thread_id_ret,
        turn_id: turn_id_ret,
    })
}

#[tauri::command]
pub async fn cancel_turn(
    state: State<'_, DesktopState>,
    thread_id: String,
    turn_id: String,
) -> Result<(), String> {
    state
        .runtime()
        .cancel_turn(&thread_id, &turn_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn steer_turn(
    state: State<'_, DesktopState>,
    thread_id: String,
    turn_id: String,
    message: String,
) -> Result<(), String> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Err("Steering message cannot be empty".to_string());
    }

    state
        .runtime()
        .steer_turn(&thread_id, &turn_id, trimmed)
        .map_err(|e| e.to_string())
}

// =============================================================================
// Pending Interaction Recovery
// =============================================================================

#[tauri::command]
pub async fn get_pending_interactions(
    state: State<'_, DesktopState>,
) -> Result<PendingInteractionsDto, String> {
    let runtime = state.runtime();
    Ok(PendingInteractionsDto {
        approvals: runtime
            .pending_approval_snapshots()
            .into_iter()
            .map(|item| PendingApprovalDto {
                request_id: item.request_id,
                thread_id: item.thread_id,
                turn_id: item.turn_id,
                command: item.command,
                reason: item.reason,
            })
            .collect(),
        questions: runtime
            .pending_question_snapshots()
            .into_iter()
            .map(|item| PendingQuestionDto {
                question_id: item.question_id,
                thread_id: item.thread_id,
                turn_id: item.turn_id,
                question: item.question,
                options: item.options,
                multi_select: item.multi_select,
            })
            .collect(),
    })
}

// =============================================================================
// Approval & Question Commands
// =============================================================================

#[tauri::command]
pub async fn respond_approval(
    state: State<'_, DesktopState>,
    request_id: String,
    decision: bool,
    reason: Option<String>,
) -> Result<(), String> {
    let approval_decision = if decision {
        ApprovalDecision::AllowOnce
    } else {
        ApprovalDecision::Deny {
            reason: reason.unwrap_or_else(|| "Rejected by user via desktop interface".to_string()),
        }
    };

    state
        .runtime()
        .respond_approval(&request_id, approval_decision)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn respond_question(
    state: State<'_, DesktopState>,
    question_id: String,
    answers: Vec<String>,
) -> Result<(), String> {
    state
        .runtime()
        .respond_question(&question_id, answers)
        .map_err(|e| e.to_string())
}
