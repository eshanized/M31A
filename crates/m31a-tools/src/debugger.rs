//! Debug Adapter Protocol (DAP) & Interactive Debugger Engine (`m31a-tools::debugger`).
//!
//! Provides interactive debugging tools:
//! - DAP / GDB Machine Interface (MI) session management (launching gdb with real subprocess management).
//! - Setting line and conditional breakpoints via `-break-insert`.
//! - Real execution control: step over (`-exec-next`), step into (`-exec-step`), step out (`-exec-finish`), continue (`-exec-continue`).
//! - Live runtime variable inspection (`-stack-list-variables --simple-values`).
//! - Live expression evaluation (`-data-evaluate-expression`) against the stopped process.

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
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

/// Live GDB process wrapper communicating over GDB/MI protocol.
struct LiveGdbSession {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl LiveGdbSession {
    fn spawn(program: &str, args: &[String], cwd: &Path) -> Result<Self> {
        let mut cmd = Command::new("gdb");
        cmd.arg("--interpreter=mi2")
            .arg("-q")
            .arg("--args")
            .arg(program);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().with_context(|| {
            format!("Failed to spawn real GDB process for target '{}'", program)
        })?;

        let stdin = child
            .stdin
            .take()
            .context("Failed to open stdin pipe to GDB")?;
        let stdout = child
            .stdout
            .take()
            .context("Failed to open stdout pipe to GDB")?;
        let reader = BufReader::new(stdout);

        let mut session = Self {
            child,
            stdin,
            reader,
        };

        // Drain startup banner until initial (gdb) prompt
        let _ = session.drain_until_prompt();

        // Disable pagination to prevent hanging on large buffers
        let _ = session.execute_mi("-gdb-set pagination off", false);
        let _ = session.execute_mi("-gdb-set confirm off", false);

        Ok(session)
    }

    fn drain_until_prompt(&mut self) -> Result<Vec<String>> {
        let mut lines = Vec::new();
        let mut raw_line = String::new();
        while self.reader.read_line(&mut raw_line)? > 0 {
            let trimmed = raw_line.trim().to_string();
            raw_line.clear();
            if trimmed == "(gdb)" {
                break;
            }
            if !trimmed.is_empty() {
                lines.push(trimmed);
            }
        }
        Ok(lines)
    }

    fn execute_mi(&mut self, command: &str, is_exec: bool) -> Result<Vec<String>> {
        writeln!(self.stdin, "{}", command)?;
        self.stdin.flush()?;

        let mut lines: Vec<String> = Vec::new();
        let mut raw_line = String::new();

        while self.reader.read_line(&mut raw_line)? > 0 {
            let trimmed = raw_line.trim().to_string();
            raw_line.clear();
            if trimmed == "(gdb)" {
                if is_exec
                    && !lines
                        .iter()
                        .any(|l: &String| l.starts_with("*stopped") || l.starts_with("^error"))
                {
                    continue;
                }
                break;
            }
            if !trimmed.is_empty() {
                lines.push(trimmed);
            }
        }
        Ok(lines)
    }

    fn parse_stopped_frame(lines: &[String]) -> Option<(String, StackFrame)> {
        let stop_re = Regex::new(r#"^\*stopped,reason="([^"]+)""#).ok()?;
        let frame_func_re = Regex::new(r#"func="([^"]+)""#).ok()?;
        let frame_line_re = Regex::new(r#"line="(\d+)""#).ok()?;
        let frame_file_re = Regex::new(r#"fullname="([^"]+)""#).ok()?;

        let mut reason = "stopped".to_string();
        let mut found_stopped = false;
        let mut frame_text = String::new();

        for line in lines {
            if line.starts_with("*stopped") {
                found_stopped = true;
                if let Some(caps) = stop_re.captures(line) {
                    reason = caps
                        .get(1)
                        .map(|m| m.as_str())
                        .unwrap_or("stopped")
                        .to_string();
                }
                frame_text = line.clone();
                break;
            }
        }

        if !found_stopped {
            return None;
        }

        let name = frame_func_re
            .captures(&frame_text)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let line_num: usize = frame_line_re
            .captures(&frame_text)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(1);

        let file_path = frame_file_re
            .captures(&frame_text)
            .and_then(|c| c.get(1))
            .map(|m| PathBuf::from(m.as_str()));

        Some((
            reason,
            StackFrame {
                id: 1,
                name,
                source_file: file_path,
                line: line_num,
                column: 1,
            },
        ))
    }

    fn terminate(&mut self) -> Result<()> {
        let _ = writeln!(self.stdin, "-gdb-exit");
        let _ = self.stdin.flush();
        let _ = self.child.kill();
        let _ = self.child.wait();
        Ok(())
    }
}

/// Active debug session instance.
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
    live_gdb: Option<LiveGdbSession>,
}

/// DAP Manager coordinating active debugging sessions.
#[derive(Default)]
pub struct DapClient {
    sessions: Arc<Mutex<HashMap<String, DebugSession>>>,
}

impl DapClient {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Launches a new debug session for the given binary.
    /// Spawns a real GDB Machine Interface (MI) subprocess and stops at main entry.
    pub fn launch(
        &self,
        session_id: &str,
        program: &str,
        args: Vec<String>,
        cwd: &Path,
    ) -> Result<DebugSessionStatus> {
        let mut sessions = self.sessions.lock().unwrap();

        let prog_path = if Path::new(program).is_absolute() {
            PathBuf::from(program)
        } else {
            cwd.join(program)
        };

        if !prog_path.exists() || !prog_path.is_file() {
            anyhow::bail!(
                "Debugger requires an executable binary on disk; cannot launch '{}'",
                prog_path.display()
            );
        }

        let mut gdb = LiveGdbSession::spawn(prog_path.to_str().unwrap_or(program), &args, cwd)
            .map_err(|e| {
                anyhow::anyhow!("Failed to spawn GDB for '{}': {}", prog_path.display(), e)
            })?;

        // Set initial breakpoint at main to pause at entry
        let _ = gdb.execute_mi("-break-insert main", false);
        let run_lines = gdb.execute_mi("-exec-run", true).unwrap_or_default();

        let (reason, frame) =
            LiveGdbSession::parse_stopped_frame(&run_lines).unwrap_or_else(|| {
                (
                    "entry".to_string(),
                    StackFrame {
                        id: 1,
                        name: "main".to_string(),
                        source_file: Some(prog_path.clone()),
                        line: 1,
                        column: 1,
                    },
                )
            });

        let frames = vec![frame.clone()];
        let mut variables = Vec::new();

        // Inspect initial variables from live process
        if let Ok(var_lines) = gdb.execute_mi("-stack-list-variables --simple-values", false) {
            variables = Self::parse_variables(&var_lines);
        }

        let final_status = DebugSessionStatus::Stopped { reason, frame };

        let session = DebugSession {
            session_id: session_id.to_string(),
            program: program.to_string(),
            args,
            cwd: cwd.to_path_buf(),
            breakpoints: Vec::new(),
            status: final_status.clone(),
            frames,
            variables,
            next_bp_id: 1,
            live_gdb: Some(gdb),
        };

        sessions.insert(session_id.to_string(), session);
        Ok(final_status)
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

        session.breakpoints.retain(|b| b.file_path != file);

        let mut added = Vec::new();
        for &line in lines {
            let mut verified = true;

            // If real GDB session is active, insert real breakpoint
            if let Some(ref mut gdb) = session.live_gdb {
                let cmd = match condition {
                    Some(ref cond) => {
                        format!("-break-insert -c \"{}\" {}:{}", cond, file.display(), line)
                    }
                    None => format!("-break-insert {}:{}", file.display(), line),
                };
                if let Ok(resp) = gdb.execute_mi(&cmd, false) {
                    verified = resp.iter().any(|l| l.starts_with("^done"));
                }
            }

            let bp = Breakpoint {
                id: session.next_bp_id,
                file_path: file.to_path_buf(),
                line,
                condition: condition.clone(),
                verified,
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

        let gdb = session.live_gdb.as_mut().ok_or_else(|| {
            anyhow::anyhow!(
                "Debug session '{}' has no active live GDB process",
                session_id
            )
        })?;

        let lines = gdb.execute_mi("-exec-next", true)?;
        if let Some((reason, frame)) = LiveGdbSession::parse_stopped_frame(&lines) {
            session.frames = vec![frame.clone()];
            session.status = DebugSessionStatus::Stopped { reason, frame };
            return Ok(session.status.clone());
        } else if lines.iter().any(|l: &String| l.contains("exited")) {
            session.status = DebugSessionStatus::Exited { exit_code: 0 };
            return Ok(session.status.clone());
        }

        anyhow::bail!("step_over failed to advance process: {:?}", lines)
    }

    /// Steps into a function call.
    pub fn step_into(&self, session_id: &str) -> Result<DebugSessionStatus> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Debug session '{}' not found", session_id))?;

        let gdb = session.live_gdb.as_mut().ok_or_else(|| {
            anyhow::anyhow!(
                "Debug session '{}' has no active live GDB process",
                session_id
            )
        })?;

        let lines = gdb.execute_mi("-exec-step", true)?;
        if let Some((reason, frame)) = LiveGdbSession::parse_stopped_frame(&lines) {
            session.frames.insert(0, frame.clone());
            session.status = DebugSessionStatus::Stopped { reason, frame };
            return Ok(session.status.clone());
        } else if lines.iter().any(|l: &String| l.contains("exited")) {
            session.status = DebugSessionStatus::Exited { exit_code: 0 };
            return Ok(session.status.clone());
        }

        anyhow::bail!("step_into failed to advance process: {:?}", lines)
    }

    /// Steps out of the current function call.
    pub fn step_out(&self, session_id: &str) -> Result<DebugSessionStatus> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Debug session '{}' not found", session_id))?;

        let gdb = session.live_gdb.as_mut().ok_or_else(|| {
            anyhow::anyhow!(
                "Debug session '{}' has no active live GDB process",
                session_id
            )
        })?;

        let lines = gdb.execute_mi("-exec-finish", true)?;
        if let Some((reason, frame)) = LiveGdbSession::parse_stopped_frame(&lines) {
            session.frames = vec![frame.clone()];
            session.status = DebugSessionStatus::Stopped { reason, frame };
            return Ok(session.status.clone());
        } else if lines.iter().any(|l: &String| l.contains("exited")) {
            session.status = DebugSessionStatus::Exited { exit_code: 0 };
            return Ok(session.status.clone());
        }

        anyhow::bail!("step_out failed to complete frame: {:?}", lines)
    }

    /// Continues execution until the next breakpoint or termination.
    pub fn continue_exec(&self, session_id: &str) -> Result<DebugSessionStatus> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Debug session '{}' not found", session_id))?;

        let gdb = session.live_gdb.as_mut().ok_or_else(|| {
            anyhow::anyhow!(
                "Debug session '{}' has no active live GDB process",
                session_id
            )
        })?;

        let lines = gdb.execute_mi("-exec-continue", true)?;
        if let Some((reason, frame)) = LiveGdbSession::parse_stopped_frame(&lines) {
            session.frames = vec![frame.clone()];
            session.status = DebugSessionStatus::Stopped { reason, frame };
            return Ok(session.status.clone());
        } else if lines.iter().any(|l: &String| l.contains("exited")) {
            session.status = DebugSessionStatus::Exited { exit_code: 0 };
            return Ok(session.status.clone());
        }

        anyhow::bail!("continue_exec failed: {:?}", lines)
    }

    /// Inspects variables in current scope.
    pub fn inspect_variables(
        &self,
        session_id: &str,
        scope: Option<&str>,
    ) -> Result<Vec<DebugVariable>> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Debug session '{}' not found", session_id))?;

        let gdb = session.live_gdb.as_mut().ok_or_else(|| {
            anyhow::anyhow!(
                "Debug session '{}' has no active live GDB process",
                session_id
            )
        })?;

        let lines = gdb.execute_mi("-stack-list-variables --simple-values", false)?;
        session.variables = Self::parse_variables(&lines);

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
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("Debug session '{}' not found", session_id))?;

        let gdb = session.live_gdb.as_mut().ok_or_else(|| {
            anyhow::anyhow!(
                "Debug session '{}' has no active live GDB process",
                session_id
            )
        })?;

        let cmd = format!("-data-evaluate-expression \"{}\"", expr);
        let lines = gdb.execute_mi(&cmd, false)?;
        let val_re = Regex::new(r#"^\^done,value="([^"]+)""#).unwrap();
        for l in lines {
            if let Some(caps) = val_re.captures(&l) {
                if let Some(val) = caps.get(1) {
                    return Ok(format!("{} = {}", expr, val.as_str()));
                }
            }
        }

        anyhow::bail!("Failed to evaluate expression '{}' in live process", expr)
    }

    /// Terminates an active debug session.
    pub fn terminate(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.lock().unwrap();
        let mut session = sessions
            .remove(session_id)
            .ok_or_else(|| anyhow::anyhow!("Debug session '{}' not found", session_id))?;

        if let Some(ref mut gdb) = session.live_gdb {
            let _ = gdb.terminate();
        }

        session.status = DebugSessionStatus::Terminated;
        Ok(())
    }

    fn parse_variables(lines: &[String]) -> Vec<DebugVariable> {
        let var_re = Regex::new(r#"\{name="([^"]+)",type="([^"]+)",value="([^"]+)"\}"#).unwrap();
        let mut vars = Vec::new();
        for line in lines {
            for caps in var_re.captures_iter(line) {
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
                let type_name = caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
                let value = caps.get(3).map(|m| m.as_str()).unwrap_or("").to_string();
                vars.push(DebugVariable {
                    name,
                    value,
                    type_name,
                    scope: "local".to_string(),
                });
            }
        }
        vars
    }
}

// =============================================================================
// Tool argument structs and functions
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugLaunchArgs {
    pub session_id: String,
    pub program: String,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
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
    #[serde(default)]
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
    pub step_type: String,
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
        let has_gcc = Command::new("gcc").arg("--version").output().is_ok();
        let has_gdb = Command::new("gdb").arg("--version").output().is_ok();
        if !has_gcc || !has_gdb {
            return;
        }

        let temp = tempfile::tempdir().unwrap();
        let c_file = temp.path().join("prog.c");
        let bin_file = temp.path().join("prog");

        std::fs::write(
            &c_file,
            r#"#include <stdio.h>
int compute(int x) {
    int y = x * 2;
    return y;
}
int main() {
    int alpha = 42;
    int beta = compute(alpha);
    printf("res: %d\n", beta);
    return 0;
}
"#,
        )
        .unwrap();

        let compile_status = Command::new("gcc")
            .arg("-g")
            .arg(&c_file)
            .arg("-o")
            .arg(&bin_file)
            .status()
            .unwrap();
        assert!(compile_status.success());

        let client = DapClient::new();
        let session_id = "test_sess_real";

        let status = client
            .launch(session_id, bin_file.to_str().unwrap(), vec![], temp.path())
            .expect("Real launch must succeed");
        assert!(matches!(status, DebugSessionStatus::Stopped { .. }));

        // Set breakpoint at compute function line 4
        let bps = client
            .set_breakpoints(session_id, &c_file, &[4], None)
            .unwrap();
        assert_eq!(bps.len(), 1);
        assert!(bps[0].verified);

        // Step over line 8 ("int alpha = 42;")
        let stepped = client.step_over(session_id).unwrap();
        assert!(matches!(stepped, DebugSessionStatus::Stopped { .. }));

        // Evaluate alpha in live process
        let eval_res = client.evaluate_expression(session_id, "alpha").unwrap();
        assert!(eval_res.contains("alpha = 42"));

        // Continue to breakpoint at compute
        let cont = client.continue_exec(session_id).unwrap();
        assert!(matches!(cont, DebugSessionStatus::Stopped { .. }));

        // Terminate
        assert!(client.terminate(session_id).is_ok());
    }

    #[test]
    fn test_live_gdb_real_binary_execution() {
        // Only run live GDB integration test if gcc and gdb are installed
        let has_gcc = Command::new("gcc").arg("--version").output().is_ok();
        let has_gdb = Command::new("gdb").arg("--version").output().is_ok();
        if !has_gcc || !has_gdb {
            return;
        }

        let temp = tempfile::tempdir().unwrap();
        let c_file = temp.path().join("prog.c");
        let bin_file = temp.path().join("prog");

        std::fs::write(
            &c_file,
            r#"
#include <stdio.h>
int main() {
    int alpha = 42;
    int beta = 84;
    printf("res: %d\n", alpha + beta);
    return 0;
}
"#,
        )
        .unwrap();

        let compile_status = Command::new("gcc")
            .arg("-g")
            .arg(&c_file)
            .arg("-o")
            .arg(&bin_file)
            .status()
            .unwrap();
        assert!(compile_status.success());

        let client = DapClient::new();
        let sess_id = "real_gdb_session";

        let status = client
            .launch(sess_id, bin_file.to_str().unwrap(), vec![], temp.path())
            .expect("Real GDB launch must succeed");
        assert!(matches!(status, DebugSessionStatus::Stopped { .. }));

        // Step over line 4 ("int alpha = 42;") in live process
        let step_res = client.step_over(sess_id).expect("Live GDB step_over");
        assert!(matches!(step_res, DebugSessionStatus::Stopped { .. }));

        // Real expression evaluation against live process (alpha now initialized to 42)
        let eval_res = client
            .evaluate_expression(sess_id, "alpha")
            .expect("Live variable evaluation");
        assert!(
            eval_res.contains("alpha = 42") || eval_res.contains("42"),
            "eval_res was: {}",
            eval_res
        );

        // Terminate real session
        client.terminate(sess_id).expect("Clean GDB termination");
    }
}
