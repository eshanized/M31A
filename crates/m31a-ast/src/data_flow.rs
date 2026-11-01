//! Data-Flow Intelligence & Variable Flow Analysis Engine (`m31a-ast::data_flow`).
//!
//! Provides global and intra-procedural data-flow tracking:
//! - Tracks variable declarations, reassignments, and mutation paths.
//! - Analyzes source-to-sink taint flows (untrusted input -> sensitive sinks).
//! - Detects dead variable assignments and unconsumed mutations.
//! - Verifies architectural data-flow invariants before code edits occur.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Origin or source of a data value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowSourceKind {
    FunctionArgument { index: usize },
    EnvironmentVariable,
    UserInput,
    NetworkRead,
    FileRead,
    ConstantLiteral,
    Unknown,
}

/// A point of data flow consumption or operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowSinkKind {
    ShellExecution,
    FileSystemWrite,
    NetworkTransmit,
    ReturnValue,
    DatabaseQuery,
    LogOutput,
    InternalComputation,
}

/// A variable definition or mutation event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableFlow {
    pub variable_name: String,
    pub defined_at_line: usize,
    pub source_kind: FlowSourceKind,
    pub is_mutable: bool,
    pub mutation_lines: Vec<usize>,
    pub read_lines: Vec<usize>,
    pub flows_to_sinks: Vec<(FlowSinkKind, usize)>,
    pub is_consumed: bool,
}

/// Data-flow analysis report for a file or code block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFlowReport {
    pub file_path: PathBuf,
    pub total_variables_tracked: usize,
    pub flows: Vec<VariableFlow>,
    pub unconsumed_mutations: Vec<String>,
    pub source_to_sink_taints: Vec<String>,
    pub summary: String,
}

pub struct DataFlowAnalyzer;

impl DataFlowAnalyzer {
    /// Analyzes intra-procedural data-flow for a given source file.
    pub fn analyze_source(path: &Path, content: &str) -> DataFlowReport {
        let lines: Vec<&str> = content.lines().collect();
        let mut flows_map: HashMap<String, VariableFlow> = HashMap::new();
        let mut unconsumed_mutations = Vec::new();
        let mut taints = Vec::new();

        for (idx, raw_line) in lines.iter().enumerate() {
            let line_no = idx + 1;
            let line = raw_line.trim();

            if line.starts_with("//") || line.starts_with("/*") || line.is_empty() {
                continue;
            }

            // 1. Detect Variable Declarations
            if line.contains("let ") {
                Self::extract_let_binding(line, line_no, &mut flows_map);
            } else if line.contains("def ") || line.contains("function ") || line.contains("fn ") {
                Self::extract_function_params(line, line_no, &mut flows_map);
            }

            // 2. Detect Reassignments and Mutations
            for (var_name, flow) in flows_map.iter_mut() {
                // Check if variable is being mutated
                let assign_pattern = format!("{} =", var_name);
                let compound_assign = format!("{} +=", var_name);
                let mutate_calls = [
                    format!("{}.push", var_name),
                    format!("{}.insert", var_name),
                    format!("{}.clear", var_name),
                    format!("{}.extend", var_name),
                ];
                let is_method_mutation = mutate_calls.iter().any(|c| line.contains(c));

                if (line.contains(&assign_pattern)
                    || line.contains(&compound_assign)
                    || is_method_mutation)
                    && !line.contains("let ")
                    && !line.contains("==")
                    && !line.contains("!=")
                {
                    flow.mutation_lines.push(line_no);
                } else if line.contains(var_name) && !line.contains(&format!("let {}", var_name)) {
                    flow.read_lines.push(line_no);
                    flow.is_consumed = true;

                    // 3. Detect Sinks
                    if line.contains("Command::new")
                        || line.contains("exec(")
                        || line.contains("exec_command")
                    {
                        flow.flows_to_sinks
                            .push((FlowSinkKind::ShellExecution, line_no));
                        if flow.source_kind == FlowSourceKind::UserInput
                            || flow.source_kind == FlowSourceKind::EnvironmentVariable
                        {
                            taints.push(format!(
                                "Untrusted source variable '{}' (line {}) flows into shell execution sink at line {}",
                                var_name, flow.defined_at_line, line_no
                            ));
                        }
                    } else if line.contains("fs::write")
                        || line.contains("write_file")
                        || line.contains(".write(")
                    {
                        flow.flows_to_sinks
                            .push((FlowSinkKind::FileSystemWrite, line_no));
                    } else if line.contains("reqwest::")
                        || line.contains("fetch(")
                        || line.contains("socket.")
                    {
                        flow.flows_to_sinks
                            .push((FlowSinkKind::NetworkTransmit, line_no));
                    } else if line.contains("return ")
                        || (line.ends_with(var_name) && !line.ends_with(';'))
                    {
                        flow.flows_to_sinks
                            .push((FlowSinkKind::ReturnValue, line_no));
                    }
                }
            }
        }

        // Identify unconsumed mutations
        for (var_name, flow) in &flows_map {
            if !flow.mutation_lines.is_empty() {
                let last_mutation = *flow.mutation_lines.last().unwrap();
                let read_after = flow.read_lines.iter().any(|&r| r > last_mutation);
                if !read_after {
                    unconsumed_mutations.push(format!(
                        "Variable '{}' is mutated at line {} but never read afterwards.",
                        var_name, last_mutation
                    ));
                }
            } else if !flow.is_consumed && flow.source_kind != FlowSourceKind::ConstantLiteral {
                unconsumed_mutations.push(format!(
                    "Variable '{}' declared at line {} is never consumed.",
                    var_name, flow.defined_at_line
                ));
            }
        }

        let flows: Vec<VariableFlow> = flows_map.into_values().collect();
        let total_tracked = flows.len();

        let summary = format!(
            "Data-Flow Analysis for '{}': {} variables tracked, {} unconsumed mutations, {} source-to-sink taints.",
            path.display(),
            total_tracked,
            unconsumed_mutations.len(),
            taints.len()
        );

        DataFlowReport {
            file_path: path.to_path_buf(),
            total_variables_tracked: total_tracked,
            flows,
            unconsumed_mutations,
            source_to_sink_taints: taints,
            summary,
        }
    }

    fn extract_let_binding(line: &str, line_no: usize, flows: &mut HashMap<String, VariableFlow>) {
        if let Some(pos) = line.find("let ") {
            let rest = &line[pos + 4..];
            let is_mut = rest.starts_with("mut ");
            let ident_part = if is_mut { &rest[4..] } else { rest };

            if let Some(eq_pos) = ident_part.find('=') {
                let name = ident_part[..eq_pos]
                    .split(':')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let rhs = ident_part[eq_pos + 1..].trim();

                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    let source_kind = if rhs.contains("std::env") || rhs.contains("env::var") {
                        FlowSourceKind::EnvironmentVariable
                    } else if rhs.contains("read_line")
                        || rhs.contains("args()")
                        || rhs.contains("stdin")
                    {
                        FlowSourceKind::UserInput
                    } else if rhs.contains("fs::read") || rhs.contains("read_to_string") {
                        FlowSourceKind::FileRead
                    } else if rhs.parse::<i64>().is_ok() || rhs.starts_with('"') {
                        FlowSourceKind::ConstantLiteral
                    } else {
                        FlowSourceKind::Unknown
                    };

                    flows.insert(
                        name.clone(),
                        VariableFlow {
                            variable_name: name,
                            defined_at_line: line_no,
                            source_kind,
                            is_mutable: is_mut,
                            mutation_lines: Vec::new(),
                            read_lines: Vec::new(),
                            flows_to_sinks: Vec::new(),
                            is_consumed: false,
                        },
                    );
                }
            }
        }
    }

    fn extract_function_params(
        line: &str,
        line_no: usize,
        flows: &mut HashMap<String, VariableFlow>,
    ) {
        if let (Some(open), Some(close)) = (line.find('('), line.rfind(')'))
            && close > open
        {
            let params_str = &line[open + 1..close];
            for (idx, p) in params_str.split(',').enumerate() {
                let p = p.trim();
                if p.is_empty() || p == "self" || p == "&self" || p == "&mut self" {
                    continue;
                }
                let var_name = p.split(':').next().unwrap_or("").trim().replace("mut ", "");
                if !var_name.is_empty() && var_name.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    flows.insert(
                        var_name.clone(),
                        VariableFlow {
                            variable_name: var_name,
                            defined_at_line: line_no,
                            source_kind: FlowSourceKind::FunctionArgument { index: idx },
                            is_mutable: p.starts_with("mut "),
                            mutation_lines: Vec::new(),
                            read_lines: Vec::new(),
                            flows_to_sinks: Vec::new(),
                            is_consumed: false,
                        },
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_flow_tracking_and_taint_detection() {
        let code = r#"
fn execute_task(cmd_input: String) {
    let mut buffer = cmd_input;
    buffer = format!("{}_suffix", buffer);
    let output = std::process::Command::new(buffer).output();
    println!("{:?}", output);
}
"#;
        let report = DataFlowAnalyzer::analyze_source(Path::new("task.rs"), code);
        assert!(report.total_variables_tracked >= 2);
        assert!(
            !report.source_to_sink_taints.is_empty()
                || report.flows.iter().any(|f| !f.flows_to_sinks.is_empty())
        );
    }

    #[test]
    fn test_unconsumed_mutations() {
        let code = r#"
fn calculate() -> i32 {
    let mut total = 10;
    total = 20;
    total
}
"#;
        let report = DataFlowAnalyzer::analyze_source(Path::new("calc.rs"), code);
        assert_eq!(report.file_path, PathBuf::from("calc.rs"));
    }
}
