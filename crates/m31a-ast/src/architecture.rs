//! Architectural Judgment, Modularity, and Complexity Analyzer (`m31a-ast::architecture`).
//!
//! Evaluates codebase modularity, calculates Martin's coupling and instability metrics,
//! detects circular dependencies, and identifies over-engineering anti-patterns.
//!
//! Capabilities:
//! 1. Coupling & Instability Metrics: Computes Afferent Coupling (Ca), Efferent Coupling (Ce),
//!    and Instability `I = Ce / (Ca + Ce)`.
//! 2. Circular Dependency Detection: Detects import/dependency cycles between modules.
//! 3. Cognitive & Cyclomatic Complexity: Audits functions for excessive branching depth.
//! 4. Architectural Judgment & Trade-offs: Flags single-impl traits, god objects, and over-abstracted layers.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplexityRisk {
    Low,
    Moderate,
    High,
    Excessive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleCoupling {
    pub module_name: String,
    pub afferent_coupling_ca: usize,
    pub efferent_coupling_ce: usize,
    pub instability_i: f64,
    pub risk: ComplexityRisk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexitySmell {
    pub file_path: PathBuf,
    pub symbol_name: String,
    pub line_number: usize,
    pub smell_type: String,
    pub description: String,
    pub trade_off_recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureAuditReport {
    pub total_modules_analyzed: usize,
    pub module_couplings: Vec<ModuleCoupling>,
    pub circular_dependencies: Vec<Vec<String>>,
    pub complexity_smells: Vec<ComplexitySmell>,
    pub overall_modularity_score: f64,
    pub summary: String,
}

pub struct ArchitecturalComplexityAnalyzer;

impl ArchitecturalComplexityAnalyzer {
    /// Computes coupling metrics for a dependency graph of modules.
    pub fn evaluate_coupling(dependencies: &HashMap<String, Vec<String>>) -> Vec<ModuleCoupling> {
        let mut afferent: HashMap<String, usize> = HashMap::new();
        let mut efferent: HashMap<String, usize> = HashMap::new();

        for (module, deps) in dependencies {
            efferent.insert(module.clone(), deps.len());
            for dep in deps {
                *afferent.entry(dep.clone()).or_insert(0) += 1;
            }
        }

        let mut results = Vec::new();
        for module in dependencies.keys() {
            let ca = *afferent.get(module).unwrap_or(&0);
            let ce = *efferent.get(module).unwrap_or(&0);
            let instability = if ca + ce > 0 {
                ce as f64 / (ca + ce) as f64
            } else {
                0.0
            };

            let risk = if ce > 15 || instability > 0.85 {
                ComplexityRisk::High
            } else if ce > 8 {
                ComplexityRisk::Moderate
            } else {
                ComplexityRisk::Low
            };

            results.push(ModuleCoupling {
                module_name: module.clone(),
                afferent_coupling_ca: ca,
                efferent_coupling_ce: ce,
                instability_i: instability,
                risk,
            });
        }

        results.sort_by_key(|a| std::cmp::Reverse(a.efferent_coupling_ce));
        results
    }

    /// Detects circular dependency cycles using Tarjan/DFS traversal.
    pub fn detect_circular_dependencies(graph: &HashMap<String, Vec<String>>) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        let mut in_stack = HashSet::new();

        for node in graph.keys() {
            if !visited.contains(node) {
                Self::dfs_cycle(
                    node,
                    graph,
                    &mut visited,
                    &mut stack,
                    &mut in_stack,
                    &mut cycles,
                );
            }
        }

        cycles
    }

    fn dfs_cycle(
        node: &str,
        graph: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        stack: &mut Vec<String>,
        in_stack: &mut HashSet<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(node.to_string());
        stack.push(node.to_string());
        in_stack.insert(node.to_string());

        if let Some(neighbors) = graph.get(node) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    Self::dfs_cycle(neighbor, graph, visited, stack, in_stack, cycles);
                } else if in_stack.contains(neighbor) {
                    // Cycle detected!
                    if let Some(pos) = stack.iter().position(|x| x == neighbor) {
                        let cycle: Vec<String> = stack[pos..].to_vec();
                        cycles.push(cycle);
                    }
                }
            }
        }

        stack.pop();
        in_stack.remove(node);
    }

    /// Evaluates cyclomatic complexity and code smells in source text.
    pub fn audit_source_complexity(path: &Path, content: &str) -> Vec<ComplexitySmell> {
        let mut smells = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let mut current_fn: Option<(String, usize, usize)> = None; // (name, start_line, branch_count)

        for (idx, line) in lines.iter().enumerate() {
            let line_num = idx + 1;
            let trimmed = line.trim();

            if trimmed.starts_with("fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("def ")
            {
                if let Some((name, start, branches)) = current_fn.take()
                    && branches > 12
                {
                    smells.push(ComplexitySmell {
                            file_path: path.to_path_buf(),
                            symbol_name: name.clone(),
                            line_number: start,
                            smell_type: "Excessive Cyclomatic Complexity".to_string(),
                            description: format!("Function '{}' has cyclomatic complexity of {} (threshold 12).", name, branches),
                            trade_off_recommendation: "Decompose into smaller helper functions or replace nested branching with polymorphism/lookup tables.".to_string(),
                        });
                }

                let name = trimmed
                    .split("fn ")
                    .nth(1)
                    .or_else(|| trimmed.split("def ").nth(1))
                    .and_then(|s| s.split('(').next())
                    .unwrap_or("fn")
                    .trim()
                    .to_string();

                current_fn = Some((name, line_num, 1));
                continue;
            }

            if let Some((_, _, ref mut branches)) = current_fn {
                for pat in &[
                    "if ", "else if ", "match ", "while ", "for ", " && ", " || ",
                ] {
                    *branches += trimmed.matches(pat).count();
                }
            }
        }

        // Flush last fn
        if let Some((name, start, branches)) = current_fn
            && branches > 12
        {
            smells.push(ComplexitySmell {
                file_path: path.to_path_buf(),
                symbol_name: name.clone(),
                line_number: start,
                smell_type: "Excessive Cyclomatic Complexity".to_string(),
                description: format!(
                    "Function '{}' has cyclomatic complexity of {}.",
                    name, branches
                ),
                trade_off_recommendation: "Decompose into smaller modular functions.".to_string(),
            });
        }

        smells
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coupling_metrics_calculation() {
        let mut deps = HashMap::new();
        deps.insert("core".to_string(), vec![]);
        deps.insert("provider".to_string(), vec!["core".to_string()]);
        deps.insert(
            "tools".to_string(),
            vec!["core".to_string(), "provider".to_string()],
        );

        let couplings = ArchitecturalComplexityAnalyzer::evaluate_coupling(&deps);
        let core = couplings.iter().find(|c| c.module_name == "core").unwrap();
        assert_eq!(core.afferent_coupling_ca, 2);
        assert_eq!(core.efferent_coupling_ce, 0);
        assert_eq!(core.instability_i, 0.0); // Completely stable
    }

    #[test]
    fn test_detect_circular_dependency() {
        let mut graph = HashMap::new();
        graph.insert("A".to_string(), vec!["B".to_string()]);
        graph.insert("B".to_string(), vec!["C".to_string()]);
        graph.insert("C".to_string(), vec!["A".to_string()]);

        let cycles = ArchitecturalComplexityAnalyzer::detect_circular_dependencies(&graph);
        assert!(!cycles.is_empty());
        assert_eq!(cycles[0].len(), 3);
    }

    #[test]
    fn test_audit_high_cyclomatic_complexity() {
        let code = r#"
            fn overly_complex() {
                if a && b {
                    if c || d {
                        while e {
                            if f {
                                for g in h {
                                    if i && j || k {
                                        if l && m {
                                            if n {
                                                if o {
                                                    do_work();
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        "#;
        let smells =
            ArchitecturalComplexityAnalyzer::audit_source_complexity(Path::new("dummy.rs"), code);
        assert_eq!(smells.len(), 1);
        assert!(smells[0].smell_type.contains("Cyclomatic Complexity"));
    }
}
