# `m31a-ast`

> Abstract Syntax Tree (AST) symbol indexing, transitive call-graph analysis, semantic blast-radius scoring, and data-flow taint tracking for M31A.

---

## Why Does This Crate Exist?

When an AI agent modifies a function signature or refactors a struct, simple text searches (grep) cannot tell you:
- Which files and crates call this function transitively?
- Is this symbol part of the public export API boundary?
- What is the probability that changing this symbol causes regressions in downstream callers?
- Are unvalidated user inputs flowing through data-flow paths into dangerous sinks?

`m31a-ast` provides deep syntactic and semantic code intelligence:
- Extracts symbols (functions, structs, traits, enums, type aliases) across Rust, TypeScript, and Python codebases.
- Builds full transitive call graphs.
- Computes a quantitative **Semantic Blast-Radius Score (0.0 to 100.0)** before patches are applied.
- Tracks intra-procedural data-flow taint to ensure untrusted inputs don't reach sensitive sinks.

---

## Core Concepts & How It Works

### 1. Symbol Codebase Indexing (`index.rs`, `parser.rs`)
Builds an in-memory graph of all codebase definitions and references, tracking visibility (`pub`, `pub(crate)`, private), parameter signatures, and documentation comments.

### 2. Semantic Blast-Radius Analysis (`impact.rs`)
Calculates the risk of modifying a symbol:
- **Direct Callers**: Files directly invoking the target symbol.
- **Transitive Callers**: Downstream modules affected through call chains.
- **Public API Boundary Check**: Changes to public crate exports carry higher risk weights.
- **Composite Risk Level**: Categorized into `Low`, `Medium`, `High`, or `Critical`.

### 3. Data-Flow Taint Tracking (`data_flow.rs`)
Tracks how variables flow through functions, identifying untrusted sources (network inputs, file reads) flowing into execution sinks without sanitization.

### 4. Semantic Versioning Drift Detection (`semver.rs`)
Detects breaking changes in public APIs to prevent accidental SemVer violations.

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/impact.rs`](src/impact.rs) | `BlastRadiusReport`, `SymbolImpact`, `ImpactRiskLevel` | Call-graph traversal and blast-radius score computation (0-100). |
| [`src/index.rs`](src/index.rs) | `AstCodebaseIndex`, `IndexedSymbol` | In-memory symbol table and reference resolver. |
| [`src/data_flow.rs`](src/data_flow.rs) | `DataFlowAnalyzer`, `TaintPath` | Intra-procedural taint analysis and sink tracking. |
| [`src/parser.rs`](src/parser.rs) | `AstParser`, `ParsedFile` | Multilingual syntax extraction (Rust, TypeScript, Python). |
| [`src/semver.rs`](src/semver.rs) | `SemverAuditor`, `ApiDiff` | Public export boundary and breaking change verification. |

---

## Practical Usage Example

```rust
use m31a_ast::impact::AstCodebaseIndex;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let mut index = AstCodebaseIndex::new();
    index.index_directory(Path::new("./crates/m31a-core"))?;

    // Analyze blast radius of modifying `Config::load`
    let report = index.analyze_blast_radius("Config::load")?;

    println!("Blast Radius Score: {:.1}/100", report.blast_radius_score);
    println!("Risk Level: {:?}", report.risk_level);
    println!("Total Affected Files: {}", report.total_affected_files);

    for impact in report.impacts {
        println!("Symbol: {} (Direct Callers: {})", impact.target_symbol, impact.direct_callers_count);
    }

    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-ast
```
