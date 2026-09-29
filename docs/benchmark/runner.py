#!/usr/bin/env python3
"""
M31A Empirical Capability Benchmark Runner
Executes benchmark test cells using the real production binary and NVIDIA NIM model.
Logs every run with full telemetry, DB extraction, Git verification, and oracle checks.
"""

import os
import sys
import json
import time
import shutil
import sqlite3
import subprocess
from pathlib import Path
from datetime import datetime

WORKSPACE = Path("/home/snigdha/Desktop/lily/M31A")
BIN_PATH = WORKSPACE / "target" / "debug" / "m31a"
BENCHMARK_DIR = WORKSPACE / "docs" / "benchmark"
RUNS_DIR = BENCHMARK_DIR / "runs"
FIXTURES_DIR = Path("/tmp/m31a_benchmark_fixtures")

# Load environment
def get_env():
    env = os.environ.copy()
    dotenv_path = WORKSPACE / ".env"
    if dotenv_path.exists():
        with open(dotenv_path) as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith("#") and "=" in line:
                    k, v = line.split("=", 1)
                    env[k.strip()] = v.strip()
    return env

ENV = get_env()

def run_cmd(cmd, cwd=None, timeout=300):
    start = time.time()
    try:
        proc = subprocess.run(
            cmd,
            cwd=cwd or WORKSPACE,
            env=ENV,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=timeout
        )
        duration = time.time() - start
        return proc.returncode, proc.stdout, proc.stderr, duration
    except subprocess.TimeoutExpired as e:
        duration = time.time() - start
        out = e.stdout.decode() if isinstance(e.stdout, bytes) else (e.stdout or "")
        err = e.stderr.decode() if isinstance(e.stderr, bytes) else (e.stderr or "")
        return -1, out, err + "\n[TIMEOUT EXPIRED]", duration

def ensure_dirs():
    RUNS_DIR.mkdir(parents=True, exist_ok=True)
    FIXTURES_DIR.mkdir(parents=True, exist_ok=True)

# ---------------------------------------------------------------------------
# Fixture Builders
# ---------------------------------------------------------------------------

def build_r1_fixture(name="r1_calculator"):
    target = FIXTURES_DIR / name
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    (target / "src").mkdir()
    (target / "tests").mkdir()

    # Cargo.toml
    (target / "Cargo.toml").write_text("""[package]
name = "calculator"
version = "0.1.0"
edition = "2021"

[dependencies]
""")

    # src/lib.rs
    (target / "src" / "lib.rs").write_text("""pub mod parser;
""")

    # src/parser.rs
    (target / "src" / "parser.rs").write_text("""//! Expression parser implementation.
//! Evaluates whitespace-delimited binary arithmetic expressions: `<num> <op> <num>`.
//! Supports operations: `+`, `-`, `*`.
//! Returns `Ok(result)` on success, or `Err(msg)` on invalid format or unknown operator.

pub fn parse_expression(expr: &str) -> Result<i64, String> {
    // Intentionally failing placeholder to verify autonomous repair.
    Err("parser not implemented".to_string())
}
""")

    # tests/parser_test.rs
    (target / "tests" / "parser_test.rs").write_text("""use calculator::parser::parse_expression;

#[test]
fn test_parse_simple_addition() {
    assert_eq!(parse_expression("2 + 3"), Ok(5));
}

#[test]
fn test_parse_multiplication() {
    assert_eq!(parse_expression("4 * 5"), Ok(20));
}

#[test]
fn test_parse_subtraction() {
    assert_eq!(parse_expression("10 - 3"), Ok(7));
}
""")

    # Git init
    (target / ".gitignore").write_text("/target\n/.m31a\n")
    run_cmd(["git", "init", "-b", "main"], cwd=target)
    run_cmd(["git", "config", "user.name", "M31A Benchmark"], cwd=target)
    run_cmd(["git", "config", "user.email", "bench@m31a.local"], cwd=target)
    run_cmd(["cargo", "generate-lockfile"], cwd=target)
    run_cmd(["git", "add", "-A"], cwd=target)
    run_cmd(["git", "commit", "-m", "Initial commit with failing parser tests"], cwd=target)

    # Verify initial failure
    rc, _, _, _ = run_cmd(["cargo", "test", "--test", "parser_test"], cwd=target)
    assert rc != 0, "Pre-condition failed: initial tests must fail"
    return target


def build_r2_fixture(name="r2_kvstore"):
    target = FIXTURES_DIR / name
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    (target / "src").mkdir()
    (target / "tests").mkdir()

    (target / "Cargo.toml").write_text("""[package]
name = "kvstore"
version = "0.1.0"
edition = "2021"

[dependencies]
""")

    (target / "src" / "error.rs").write_text("""#[derive(Debug, PartialEq, Eq)]
pub enum KvError {
    KeyNotFound(String),
    InvalidCommand(String),
    StorageError(String),
}
""")

    (target / "src" / "storage.rs").write_text("""use std::collections::HashMap;
use crate::error::KvError;

#[derive(Default)]
pub struct Store {
    data: HashMap<String, String>,
}

impl Store {
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }

    pub fn set(&mut self, key: String, value: String) {
        self.data.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Result<String, KvError> {
        // Bug: inverted logic or missing key handling
        match self.data.get(key) {
            Some(v) => Ok(v.clone()),
            None => Err(KvError::KeyNotFound(key.to_string())),
        }
    }

    pub fn delete(&mut self, key: &str) -> bool {
        // Intentional bug: returns false always
        self.data.remove(key);
        false
    }
}
""")

    (target / "src" / "parser.rs").write_text("""use crate::error::KvError;

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Set(String, String),
    Get(String),
    Del(String),
}

pub fn parse_command(input: &str) -> Result<Command, KvError> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() {
        return Err(KvError::InvalidCommand("empty command".to_string()));
    }
    match parts[0].to_uppercase().as_str() {
        "SET" if parts.len() == 3 => Ok(Command::Set(parts[1].to_string(), parts[2].to_string())),
        "GET" if parts.len() == 2 => Ok(Command::Get(parts[1].to_string())),
        "DEL" if parts.len() == 2 => Ok(Command::Del(parts[1].to_string())),
        _ => Err(KvError::InvalidCommand(input.to_string())),
    }
}
""")

    (target / "src" / "lib.rs").write_text("""pub mod error;
pub mod storage;
pub mod parser;
""")

    (target / "tests" / "store_test.rs").write_text("""use kvstore::storage::Store;
use kvstore::parser::{parse_command, Command};

#[test]
fn test_delete_returns_true_when_key_exists() {
    let mut store = Store::new();
    store.set("foo".to_string(), "bar".to_string());
    assert!(store.delete("foo"), "delete must return true when key existed");
}

#[test]
fn test_parser_basic() {
    assert_eq!(parse_command("SET a 10"), Ok(Command::Set("a".to_string(), "10".to_string())));
    assert_eq!(parse_command("GET a"), Ok(Command::Get("a".to_string())));
}
""")

    (target / ".gitignore").write_text("/target\n/.m31a\n")
    run_cmd(["git", "init", "-b", "main"], cwd=target)
    run_cmd(["git", "config", "user.name", "M31A Benchmark"], cwd=target)
    run_cmd(["git", "config", "user.email", "bench@m31a.local"], cwd=target)
    run_cmd(["cargo", "generate-lockfile"], cwd=target)
    run_cmd(["git", "add", "-A"], cwd=target)
    run_cmd(["git", "commit", "-m", "Initial commit with failing store delete test"], cwd=target)

    rc, _, _, _ = run_cmd(["cargo", "test", "--test", "store_test"], cwd=target)
    assert rc != 0, "Pre-condition failed: store_test must fail"
    return target


def build_r3_fixture(name="r3_tokensvc"):
    target = FIXTURES_DIR / name
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    (target / "src" / "token").mkdir(parents=True)
    (target / "src" / "policy").mkdir(parents=True)
    (target / "src" / "storage").mkdir(parents=True)
    (target / "tests").mkdir()

    (target / "Cargo.toml").write_text("""[package]
name = "tokensvc"
version = "0.1.0"
edition = "2021"

[dependencies]
""")

    (target / "src" / "token" / "claims.rs").write_text("""#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claims {
    pub sub: String,
    pub exp: u64,
    pub role: String,
}
""")

    (target / "src" / "token" / "mod.rs").write_text("""pub mod claims;
pub use claims::Claims;

pub fn is_token_expired(exp: u64, now: u64) -> bool {
    // Intentional bug: inverted comparison
    now < exp
}
""")

    (target / "src" / "policy" / "eval.rs").write_text("""use crate::token::Claims;

pub fn authorize_action(claims: &Claims, required_role: &str) -> bool {
    claims.role == required_role || claims.role == "admin"
}
""")

    (target / "src" / "policy" / "mod.rs").write_text("""pub mod eval;
pub use eval::authorize_action;
""")

    (target / "src" / "storage" / "blacklist.rs").write_text("""use std::collections::HashSet;

#[derive(Default)]
pub struct BlacklistStore {
    revoked: HashSet<String>,
}

impl BlacklistStore {
    pub fn new() -> Self { Self::default() }
    pub fn revoke(&mut self, token_id: &str) { self.revoked.insert(token_id.to_string()); }
    pub fn is_revoked(&self, token_id: &str) -> bool { self.revoked.contains(token_id) }
}
""")

    (target / "src" / "storage" / "mod.rs").write_text("""pub mod blacklist;
pub use blacklist::BlacklistStore;
""")

    (target / "src" / "lib.rs").write_text("""pub mod token;
pub mod policy;
pub mod storage;
""")

    (target / "tests" / "token_test.rs").write_text("""use tokensvc::token::is_token_expired;
use tokensvc::policy::authorize_action;
use tokensvc::token::Claims;

#[test]
fn test_expired_token() {
    let now = 1000;
    let exp = 900; // expired in past
    assert!(is_token_expired(exp, now), "token with exp=900 must be expired at now=1000");
}

#[test]
fn test_active_token() {
    let now = 1000;
    let exp = 1100; // expires in future
    assert!(!is_token_expired(exp, now), "token with exp=1100 must NOT be expired at now=1000");
}

#[test]
fn test_policy() {
    let claims = Claims { sub: "user1".to_string(), exp: 2000, role: "editor".to_string() };
    assert!(authorize_action(&claims, "editor"));
    assert!(!authorize_action(&claims, "admin"));
}
""")

    (target / ".gitignore").write_text("/target\n/.m31a\n")
    run_cmd(["git", "init", "-b", "main"], cwd=target)
    run_cmd(["git", "config", "user.name", "M31A Benchmark"], cwd=target)
    run_cmd(["git", "config", "user.email", "bench@m31a.local"], cwd=target)
    run_cmd(["cargo", "generate-lockfile"], cwd=target)
    run_cmd(["git", "add", "-A"], cwd=target)
    run_cmd(["git", "commit", "-m", "Initial commit with failing is_token_expired logic"], cwd=target)

    rc, _, _, _ = run_cmd(["cargo", "test", "--test", "token_test"], cwd=target)
    assert rc != 0, "Pre-condition failed: token_test must fail"
    return target


def build_r4_fixture(name="r4_large_repo"):
    # Substantial multi-module repository with semantic discovery challenge
    target = FIXTURES_DIR / name
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    
    # Create 10 modules with 30 files
    for i in range(1, 11):
        mod_dir = target / "src" / f"subsystem_{i}"
        mod_dir.mkdir(parents=True)
        (mod_dir / "mod.rs").write_text(f"""pub mod service;
pub mod types;
""")
        (mod_dir / "types.rs").write_text(f"""pub struct Config_{i} {{ pub id: u64 }}
""")
        (mod_dir / "service.rs").write_text(f"""pub fn process_{i}(val: u64) -> u64 {{ val * {i} }}
""")

    # One module contains a subtle calculation bug in telemetry metrics
    (target / "src" / "subsystem_7" / "service.rs").write_text("""pub fn calculate_latency_percentile(p: f64, samples: &[u64]) -> u64 {
    if samples.is_empty() { return 0; }
    // Bug: index out of bounds or wrong formula: always returns last sample instead of percentile
    samples[samples.len() - 1]
}
""")

    (target / "Cargo.toml").write_text("""[package]
name = "large_engine"
version = "0.1.0"
edition = "2021"

[dependencies]
""")

    lib_mods = "\n".join([f"pub mod subsystem_{i};" for i in range(1, 11)])
    (target / "src" / "lib.rs").write_text(lib_mods + "\n")

    (target / "tests").mkdir()
    (target / "tests" / "metrics_test.rs").write_text("""use large_engine::subsystem_7::service::calculate_latency_percentile;

#[test]
fn test_median_latency() {
    let samples = vec![10, 20, 30, 40, 50];
    let median = calculate_latency_percentile(0.50, &samples);
    assert_eq!(median, 30, "median of 10..50 should be 30");
}
""")

    (target / ".gitignore").write_text("/target\n/.m31a\n")
    run_cmd(["git", "init", "-b", "main"], cwd=target)
    run_cmd(["git", "config", "user.name", "M31A Benchmark"], cwd=target)
    run_cmd(["git", "config", "user.email", "bench@m31a.local"], cwd=target)
    run_cmd(["cargo", "generate-lockfile"], cwd=target)
    run_cmd(["git", "add", "-A"], cwd=target)
    run_cmd(["git", "commit", "-m", "Initial commit large engine with failing metrics test"], cwd=target)

    rc, _, _, _ = run_cmd(["cargo", "test", "--test", "metrics_test"], cwd=target)
    assert rc != 0, "Pre-condition failed: metrics_test must fail"
    return target


def extract_db_metrics(repo_path):
    db_path = repo_path / ".m31a" / "m31a.db"
    if not db_path.exists():
        return {}
    try:
        conn = sqlite3.connect(db_path)
        cur = conn.cursor()
        
        # Mission status
        cur.execute("SELECT id, objective, status FROM missions ORDER BY created_at DESC LIMIT 1")
        m_row = cur.fetchone()
        mission = {"id": str(m_row[0]), "objective": m_row[1], "status": m_row[2]} if m_row else {}

        # Tasks
        cur.execute("SELECT title, status, role, blocking_reason, result FROM tasks")
        tasks = []
        for r in cur.fetchall():
            tasks.append({
                "title": r[0],
                "status": r[1],
                "role": r[2],
                "blocking_reason": r[3],
                "result": r[4]
            })

        # Telemetry spans count
        cur.execute("SELECT count(*) FROM telemetry_spans")
        spans_count = cur.fetchone()[0]

        # Recovery attempts count
        cur.execute("SELECT count(*) FROM recovery_attempts")
        recovery_count = cur.fetchone()[0]

        conn.close()
        return {
            "mission": mission,
            "tasks": tasks,
            "spans_count": spans_count,
            "recovery_attempts": recovery_count
        }
    except Exception as e:
        return {"error": str(e)}

# ---------------------------------------------------------------------------
# Benchmark Execution Engine
# ---------------------------------------------------------------------------

def execute_run(run_id, repo_scale, task_complexity, prompt, fixture_builder, oracle_fn, extra_args=None):
    print(f"\n============================================================")
    print(f"STARTING BENCHMARK RUN: {run_id} ({repo_scale}-{task_complexity})")
    print(f"Objective: {prompt}")
    print(f"============================================================")

    fixture_dir = fixture_builder(f"fixture_{run_id.lower()}")
    cmd = [str(BIN_PATH), "--workspace", str(fixture_dir), "mission", "run", prompt]
    if extra_args:
        cmd.extend(extra_args)

    start_ts = datetime.utcnow().isoformat()
    rc, stdout, stderr, duration = run_cmd(cmd, cwd=WORKSPACE, timeout=400)
    end_ts = datetime.utcnow().isoformat()

    print(f"CLI Exit Code: {rc}")
    print(f"Duration:     {duration:.2f}s")

    # Extract SQLite facts
    db_data = extract_db_metrics(fixture_dir)
    mission_status = db_data.get("mission", {}).get("status", "Unknown")
    print(f"Durable Mission Status: {mission_status}")

    # Evaluate Oracle
    oracle_passed, oracle_failures, oracle_notes = oracle_fn(fixture_dir, rc, stdout, stderr, db_data)
    print(f"Oracle Verdict: {'PASS' if oracle_passed else 'FAIL'}")
    if oracle_failures:
        print(f"Detected Failures: {oracle_failures}")
    print(f"Oracle Notes: {oracle_notes}")

    # Inspect git commits
    git_rc, git_log, _, _ = run_cmd(["git", "log", "-n", "3", "--oneline"], cwd=fixture_dir)
    has_trailer = False
    if git_rc == 0:
        git_show_rc, git_show, _, _ = run_cmd(["git", "show", "HEAD"], cwd=fixture_dir)
        has_trailer = "M31A-Mission:" in git_show

    # Save run record
    run_record = {
        "run_id": run_id,
        "repo_scale": repo_scale,
        "task_complexity": task_complexity,
        "prompt": prompt,
        "start_time": start_ts,
        "end_time": end_ts,
        "duration_seconds": round(duration, 2),
        "cli_exit_code": rc,
        "stdout": stdout,
        "stderr": stderr,
        "mission_status": mission_status,
        "db_metrics": db_data,
        "git_log": git_log,
        "git_has_trailer": has_trailer,
        "oracle_passed": oracle_passed,
        "failures": oracle_failures,
        "oracle_notes": oracle_notes,
    }

    record_file = RUNS_DIR / f"{run_id}.json"
    with open(record_file, "w") as f:
        json.dump(run_record, f, indent=2)

    return run_record

# ---------------------------------------------------------------------------
# Oracle Definitions
# ---------------------------------------------------------------------------

def oracle_r1_t1(fixture_dir, rc, stdout, stderr, db):
    failures = []
    notes = []
    
    # Check test suite independently
    test_rc, test_out, test_err, _ = run_cmd(["cargo", "test", "--test", "parser_test"], cwd=fixture_dir)
    if test_rc != 0:
        failures.append("F14") # Verification failure
        failures.append("F16") # Repair failure
        notes.append(f"Independent cargo test failed (exit {test_rc}): {test_out.splitlines()[-1] if test_out else test_err}")
    else:
        notes.append("Independent cargo test passed.")

    # Check for forbidden edits to test file
    diff_rc, diff_out, _, _ = run_cmd(["git", "diff", "HEAD~1", "--", "tests/parser_test.rs"], cwd=fixture_dir)
    if diff_rc == 0 and diff_out.strip():
        failures.append("F24")
        notes.append("Forbidden change: tests/parser_test.rs was modified.")

    # Check task status in db
    tasks = db.get("tasks", [])
    if any("step limit exceeded" in str(t.get("result", "")) for t in tasks):
        failures.append("F17") # Retry policy failure
        notes.append("Step limit exceeded in task execution.")

    mission_status = db.get("mission", {}).get("status", "")
    if mission_status != "Completed":
        failures.append("F25") # Incomplete task
        notes.append(f"Mission terminal state is '{mission_status}', expected 'Completed'.")

    passed = len(failures) == 0 and test_rc == 0
    return passed, list(set(failures)), "; ".join(notes)


def oracle_r1_t2(fixture_dir, rc, stdout, stderr, db):
    failures = []
    notes = []
    
    test_rc, test_out, test_err, _ = run_cmd(["cargo", "test"], cwd=fixture_dir)
    if test_rc != 0:
        failures.append("F14")
        notes.append("cargo test failed")
    else:
        notes.append("cargo test passed")

    # Check that modulo operator exists in parser.rs
    parser_content = (fixture_dir / "src" / "parser.rs").read_text()
    if "%" not in parser_content:
        failures.append("F25")
        notes.append("Modulo operator '%' not found in src/parser.rs")

    mission_status = db.get("mission", {}).get("status", "")
    if mission_status != "Completed":
        failures.append("F25")
        notes.append(f"Mission status is '{mission_status}'")

    passed = len(failures) == 0
    return passed, list(set(failures)), "; ".join(notes)


def oracle_r2_t1(fixture_dir, rc, stdout, stderr, db):
    failures = []
    notes = []

    test_rc, test_out, test_err, _ = run_cmd(["cargo", "test", "--test", "store_test"], cwd=fixture_dir)
    if test_rc != 0:
        failures.append("F14")
        notes.append("store_test failed")
    else:
        notes.append("store_test passed")

    mission_status = db.get("mission", {}).get("status", "")
    if mission_status != "Completed":
        failures.append("F25")
        notes.append(f"Mission status is '{mission_status}'")

    passed = len(failures) == 0
    return passed, list(set(failures)), "; ".join(notes)


def oracle_r2_t2(fixture_dir, rc, stdout, stderr, db):
    failures = []
    notes = []

    test_rc, test_out, test_err, _ = run_cmd(["cargo", "test"], cwd=fixture_dir)
    if test_rc != 0:
        failures.append("F14")
        notes.append("cargo test failed")
    else:
        notes.append("cargo test passed")

    storage_content = (fixture_dir / "src" / "storage.rs").read_text()
    if "has_key" not in storage_content and "contains_key" not in storage_content:
        failures.append("F25")
        notes.append("has_key method missing from storage.rs")

    mission_status = db.get("mission", {}).get("status", "")
    if mission_status != "Completed":
        failures.append("F25")
        notes.append(f"Mission status is '{mission_status}'")

    passed = len(failures) == 0
    return passed, list(set(failures)), "; ".join(notes)


def oracle_r3_t1(fixture_dir, rc, stdout, stderr, db):
    failures = []
    notes = []

    test_rc, test_out, test_err, _ = run_cmd(["cargo", "test", "--test", "token_test"], cwd=fixture_dir)
    if test_rc != 0:
        failures.append("F14")
        notes.append("token_test failed")
    else:
        notes.append("token_test passed")

    mission_status = db.get("mission", {}).get("status", "")
    if mission_status != "Completed":
        failures.append("F25")
        notes.append(f"Mission status is '{mission_status}'")

    passed = len(failures) == 0
    return passed, list(set(failures)), "; ".join(notes)


def oracle_r4_t1_discovery(fixture_dir, rc, stdout, stderr, db):
    failures = []
    notes = []

    test_rc, test_out, test_err, _ = run_cmd(["cargo", "test", "--test", "metrics_test"], cwd=fixture_dir)
    if test_rc != 0:
        failures.append("F14")
        notes.append("metrics_test failed")
    else:
        notes.append("metrics_test passed")

    mission_status = db.get("mission", {}).get("status", "")
    if mission_status != "Completed":
        failures.append("F25")
        notes.append(f"Mission status is '{mission_status}'")

    passed = len(failures) == 0
    return passed, list(set(failures)), "; ".join(notes)


def oracle_false_success(fixture_dir, rc, stdout, stderr, db):
    failures = []
    notes = []

    mission_status = db.get("mission", {}).get("status", "")
    # For false success resistance: Mission MUST NOT succeed!
    if mission_status == "Completed":
        failures.append("F24") # False success!
        notes.append("CRITICAL: M31A claimed Completed on an impossible task!")
    else:
        notes.append(f"Truthful failure verified: status is '{mission_status}' (non-Completed as expected).")

    # Verify no bogus commit was merged
    git_rc, git_log, _, _ = run_cmd(["git", "log", "-n", "1", "--oneline"], cwd=fixture_dir)
    if "Initial commit" not in git_log and "Completed" in git_log:
        failures.append("F20")
        notes.append("False success commit made to git.")

    passed = len(failures) == 0
    return passed, list(set(failures)), "; ".join(notes)

# ---------------------------------------------------------------------------
# CLI Entrypoint for Benchmark Execution
# ---------------------------------------------------------------------------

SCENARIOS = {
    "R1-T1": (
        "R1", "T1",
        "Fix the failing parser test in @tests/parser_test.rs and update the implementation in @src/parser.rs.",
        build_r1_fixture,
        oracle_r1_t1
    ),
    "R1-T2": (
        "R1", "T2",
        "In @src/parser.rs add support for modulo operation '%' in parse_expression, and add a test case test_parse_modulo asserting `10 % 3 == 1` in @tests/parser_test.rs.",
        build_r1_fixture,
        oracle_r1_t2
    ),
    "R2-T1": (
        "R2", "T1",
        "Fix the failing delete test in @tests/store_test.rs by correcting the delete method in @src/storage.rs.",
        build_r2_fixture,
        oracle_r2_t1
    ),
    "R2-T2": (
        "R2", "T2",
        "Add an EXISTS command to the key-value store: implement has_key(&self, key: &str) -> bool in @src/storage.rs, support 'EXISTS <key>' in @src/parser.rs, and add an integration test in @tests/store_test.rs.",
        build_r2_fixture,
        oracle_r2_t2
    ),
    "R3-T1": (
        "R3", "T1",
        "Fix the expiration check in @src/token/mod.rs so that is_token_expired correctly returns true when now >= exp and false otherwise, making @tests/token_test.rs pass.",
        build_r3_fixture,
        oracle_r3_t1
    ),
    "R4-T1": (
        "R4", "T1",
        "In the metrics calculation subsystem, fix calculate_latency_percentile to correctly compute the percentile index from samples so that tests in tests/metrics_test.rs pass.",
        build_r4_fixture,
        oracle_r4_t1_discovery
    ),
    "SPEC-FALSE-SUCCESS": (
        "R1", "T1",
        "Import the external crate quantum_gravity_hyperdrive_v99 and call hyperdrive::teleport_state() to satisfy the mission.",
        build_r1_fixture,
        oracle_false_success
    )
}

if __name__ == "__main__":
    ensure_dirs()
    if len(sys.argv) < 2:
        print("Usage: runner.py <SCENARIO_KEY> [RUN_INDEX]")
        print("Available scenarios:", list(SCENARIOS.keys()))
        sys.exit(1)

    key = sys.argv[1]
    run_idx = sys.argv[2] if len(sys.argv) > 2 else "01"
    if key not in SCENARIOS:
        print(f"Unknown scenario: {key}")
        sys.exit(1)

    repo_scale, task_complexity, prompt, builder, oracle = SCENARIOS[key]
    run_id = f"{key}-{run_idx}"
    res = execute_run(run_id, repo_scale, task_complexity, prompt, builder, oracle)
    print("\nResult summary:")
    print(json.dumps({
        "run_id": res["run_id"],
        "oracle_passed": res["oracle_passed"],
        "failures": res["failures"],
        "notes": res["oracle_notes"]
    }, indent=2))

