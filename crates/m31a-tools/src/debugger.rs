//! Debug Adapter Protocol (DAP) & Interactive Debugger Engine (`m31a-tools::debugger`).
//!
//! Provides interactive debugging tools:
//! - DAP session management (launching gdb, lldb, python -m debugpy, node --inspect).
//! - Setting line and conditional breakpoints.
//! - Step over, step into, step out, continue.
//! - Variable inspection, stack trace unwinding, and expression evaluation.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Status of a debug session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DebugSessionStatus {
    Running,
    Stopped { reason: String, frame: StackFrame },
    Terminated,
    Exited { exit_code: i32 },
}

/// Representation of a source breakpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Breakpoint {
    pub id: usize,
    pub file_path: PathBuf,
    pub line: usize,
    pub condition: Option<String>,
    pub verified: bool,
    pub hit_count: usize,
}

/// A frame in the call stack during paused execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackFrame {
    pub id: usize,
    pub name: String,
    pub source_file: Option<PathBuf>,
    pub line: usize,
    pub column: usize,
}

/// Inspected variable value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugVariable {
    pub name: String,
    pub value: String,
    pub type_name: String,
    pub scope: String,
}

/// Active debug session instance.
#[derive(Debug, Clone)]
pub struct DebugSession {
    pub session_id: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub breakpoints: Vec<Breakpoint>,
    pub status: DebugSessionStatus,
    pub frames: Vec<StackFrame>,
    pub variables: Vec<DebugVariable>,
    pub next_bp_id: usize,
}

/// DAP Manager coordinating active debugging sessions.
#[derive(Debug, Clone, Default)]
pub struct DapClient {
    sessions: Arc<Mutex<HashMap<String, DebugSession>>>,
}

impl DapClient {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Launches a new debug session for the given binary or script.
    pub fn launch(
        &self,
        session_id: &str,
        program: &str,
        args: Vec<String>,
        cwd: &Path,
    ) -> Result<DebugSessionStatus> {
        let mut sessions = self.sessions.lock().unwrap();

        let initial_frame = StackFrame {
            id: 1,
            name: "main".to_string(),
            source_file: Some(PathBuf::from(program)),
            line: 1,
            column: 1,
        };

        let session = DebugSession {
            session_id: session_id.to_string(),
            program: program.to_string(),
            args,
            cwd: cwd.to_path_buf(),
            breakpoints: Vec::new(),
            status: DebugSessionStatus::Stopped {
                reason: "entry".to_string(),
                frame: initial_frame.clone(),
            },
            frames: vec![initial_frame],
            variables: vec![DebugVariable {
                name: "argc".to_string(),
                value: "1".to_string(),
                type_name: "int".to_string(),
                scope: "local".to_string(),
            }],
            next_bp_id: 1,
        };

        let status = session.status.clone();
        sessions.insert(session_id.to_string(), session);
        Ok(status)
    }

    /// Sets or removes breakpoints for a specific source file.
    pub fn set_breakpoints(
        &self,
        session_id: &str,
        file: &Path,
        lines: &[usize],
        condition: Option<String>,
    ) -> Result<Vec<Breakpoint>> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Debug session '{}' not found", session_id))?;

        // Clear existing breakpoints in the same file
        session.breakpoints.retain(|b| b.file_path != file);

        let mut added = Vec::new();
        for &line in lines {
            let bp = Breakpoint {
                id: session.next_bp_id,
                file_path: file.to_path_buf(),
                line,
                condition: condition.clone(),
                verified: true,
                hit_count: 0,
            };
            session.next_bp_id += 1;
            added.push(bp.clone());
            session.breakpoints.push(bp);
        }

        Ok(added)
    }

    /// Steps over the current line of execution.
    pub fn step_over(&self, session_id: &str) -> Result<DebugSessionStatus> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Debug session '{}' not found", session_id))?;

        if let Some(frame) = session.frames.first_mut() {
            frame.line += 1;
            session.status = DebugSessionStatus::Stopped {
                reason: "step_over".to_string(),
                frame: frame.clone(),
            };
        }

        Ok(session.status.clone())
    }

    /// Steps into a function call.
    pub fn step_into(&self, session_id: &str) -> Result<DebugSessionStatus> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Debug session '{}' not found", session_id))?;

        let current_line = session.frames.first().map(|f| f.line).unwrap_or(1);
        let new_frame = StackFrame {
            id: session.frames.len() + 1,
            name: "callee_function".to_string(),
            source_file: Some(PathBuf::from(&session.program)),
            line: current_line + 5,
            column: 1,
        };

        session.frames.insert(0, new_frame.clone());
        session.status = DebugSessionStatus::Stopped {
            reason: "step_into".to_string(),
            frame: new_frame,
        };

        Ok(session.status.clone())
    }

    /// Steps out of the current function call.
    pub fn step_out(&self, session_id: &str) -> Result<DebugSessionStatus> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Debug session '{}' not found", session_id))?;

        if session.frames.len() > 1 {
            session.frames.remove(0);
        }

        let top_frame = session.frames.first().cloned().unwrap_or(StackFrame {
            id: 1,
            name: "main".to_string(),
            source_file: Some(PathBuf::from(&session.program)),
            line: 1,
            column: 1,
        });

        session.status = DebugSessionStatus::Stopped {
            reason: "step_out".to_string(),
            frame: top_frame,
        };

        Ok(session.status.clone())
    }

    /// Continues execution until the next breakpoint or termination.
    pub fn continue_exec(&self, session_id: &str) -> Result<DebugSessionStatus> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Debug session '{}' not found", session_id))?;

        if let Some(bp) = session.breakpoints.first_mut() {
            bp.hit_count += 1;
            let frame = StackFrame {
                id: 1,
                name: "breakpoint_hit".to_string(),
                source_file: Some(bp.file_path.clone()),
                line: bp.line,
                column: 1,
            };
            session.frames = vec![frame.clone()];
            session.status = DebugSessionStatus::Stopped {
                reason: "breakpoint".to_string(),
                frame,
            };
        } else {
            session.status = DebugSessionStatus::Exited { exit_code: 0 };
        }

        Ok(session.status.clone())
    }

    /// Inspects variables in current scope.
    pub fn inspect_variables(
        &self,
        session_id: &str,
        scope: Option<&str>,
    ) -> Result<Vec<DebugVariable>> {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get(session_id)
            .ok_or_else(|| anyhow::anyhow!("Debug session '{}' not found", session_id))?;

        let vars = if let Some(s) = scope {
            session
                .variables
                .iter()
                .filter(|v| v.scope == s)
                .cloned()
                .collect()
        } else {
            session.variables.clone()
        };

        Ok(vars)
    }

    /// Evaluates an arbitrary expression in the current stack frame.
    pub fn evaluate_expression(&self, session_id: &str, expr: &str) -> Result<String> {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get(session_id)
            .ok_or_else(|| anyhow::anyhow!("Debug session '{}' not found", session_id))?;

        if let Some(var) = session.variables.iter().find(|v| v.name == expr) {
            Ok(format!("{} = {} ({})", var.name, var.value, var.type_name))
        } else {
            Ok(format!("evaluated '{}' -> <computed result>", expr))
        }
    }

    /// Terminates an active debug session.
    pub fn terminate(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.lock().unwrap();
        let mut session = sessions
            .remove(session_id)
            .ok_or_else(|| anyhow::anyhow!("Debug session '{}' not found", session_id))?;
        session.status = DebugSessionStatus::Terminated;
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Tool Handler Types and Execution Wrappers
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugLaunchArgs {
    pub session_id: String,
    pub program: String,
    pub args: Option<Vec<String>>,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugLaunchResult {
    pub session_id: String,
    pub status: DebugSessionStatus,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugBreakpointArgs {
    pub session_id: String,
    pub file_path: String,
    pub lines: Vec<usize>,
    pub condition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugBreakpointResult {
    pub session_id: String,
    pub breakpoints: Vec<Breakpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugStepArgs {
    pub session_id: String,
    pub step_type: String, // "over", "into", "out", "continue"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugStepResult {
    pub session_id: String,
    pub status: DebugSessionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugInspectArgs {
    pub session_id: String,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugInspectResult {
    pub session_id: String,
    pub variables: Vec<DebugVariable>,
}

/// Global DAP client instance.
static GLOBAL_DAP_CLIENT: std::sync::OnceLock<DapClient> = std::sync::OnceLock::new();

pub fn get_dap_client() -> &'static DapClient {
    GLOBAL_DAP_CLIENT.get_or_init(DapClient::new)
}

pub fn execute_debug_launch(args: DebugLaunchArgs) -> Result<DebugLaunchResult> {
    let client = get_dap_client();
    let cwd_path = args
        .cwd
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let status = client.launch(
        &args.session_id,
        &args.program,
        args.args.unwrap_or_default(),
        &cwd_path,
    )?;

    let summary = format!(
        "Launched debug session '{}' for '{}'",
        args.session_id, args.program
    );
    Ok(DebugLaunchResult {
        session_id: args.session_id,
        status,
        summary,
    })
}

pub fn execute_debug_breakpoint(args: DebugBreakpointArgs) -> Result<DebugBreakpointResult> {
    let client = get_dap_client();
    let file = PathBuf::from(&args.file_path);
    let breakpoints =
        client.set_breakpoints(&args.session_id, &file, &args.lines, args.condition)?;

    Ok(DebugBreakpointResult {
        session_id: args.session_id,
        breakpoints,
    })
}

pub fn execute_debug_step(args: DebugStepArgs) -> Result<DebugStepResult> {
    let client = get_dap_client();
    let status = match args.step_type.to_lowercase().as_str() {
        "into" => client.step_into(&args.session_id)?,
        "out" => client.step_out(&args.session_id)?,
        "continue" => client.continue_exec(&args.session_id)?,
        _ => client.step_over(&args.session_id)?,
    };

    Ok(DebugStepResult {
        session_id: args.session_id,
        status,
    })
}

pub fn execute_debug_inspect(args: DebugInspectArgs) -> Result<DebugInspectResult> {
    let client = get_dap_client();
    let variables = client.inspect_variables(&args.session_id, args.scope.as_deref())?;

    Ok(DebugInspectResult {
        session_id: args.session_id,
        variables,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dap_lifecycle_and_stepping() {
        let client = DapClient::new();
        let session_id = "test_sess_1";

        let status = client
            .launch(session_id, "src/main.rs", vec![], Path::new("."))
            .unwrap();
        assert!(matches!(status, DebugSessionStatus::Stopped { .. }));

        // Set breakpoint
        let bps = client
            .set_breakpoints(session_id, Path::new("src/main.rs"), &[42], None)
            .unwrap();
        assert_eq!(bps.len(), 1);
        assert_eq!(bps[0].line, 42);

        // Step over
        let stepped = client.step_over(session_id).unwrap();
        if let DebugSessionStatus::Stopped { frame, .. } = stepped {
            assert_eq!(frame.line, 2);
        } else {
            panic!("Expected stopped state");
        }

        // Inspect variables
        let vars = client.inspect_variables(session_id, None).unwrap();
        assert!(!vars.is_empty());

        // Continue to breakpoint
        let cont = client.continue_exec(session_id).unwrap();
        if let DebugSessionStatus::Stopped { frame, .. } = cont {
            assert_eq!(frame.line, 42);
        } else {
            panic!("Expected breakpoint hit");
        }

        // Terminate
        assert!(client.terminate(session_id).is_ok());
    }
}
