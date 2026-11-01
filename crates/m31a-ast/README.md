# `m31a-ast`

Codebase Abstract Syntax Tree (AST), symbol intelligence, semantic chunking, and blast-radius analysis engine for **M31A**.

---

## Overview

`m31a-ast` parses source code into structured syntax trees, extracts symbols (functions, structs, traits, modules), constructs transitive dependency call-graphs, and computes semantic blast-radius scores. It enables agents to understand the collateral effects of an edit before applying it.

---

## User-Facing Capabilities

- **Codebase Safety & Blast-Radius Gate (`/safety` or `m31a-verify`)**: Audits changes to prevent collateral regressions across dependent files.
- **Data-Flow Taint Tracking**: Detects unconsumed mutations, dead assignments, and untrusted inputs flowing into sensitive sinks.
- **Architectural Complexity Analysis**: Flags cyclomatic complexity spikes, tight module coupling, and dead code clones.

---

## Developer Guide & Architecture

### Key Modules

- [`architecture`](src/architecture.rs): `ArchitecturalComplexityAnalyzer` computing `ComplexityRisk`, `ComplexitySmell`, and `ModuleCoupling`.
- [`chunker`](src/chunker.rs): `AstChunker` segmenting code files along logical syntax boundaries (function/class boundaries) rather than arbitrary line counts.
- [`clones`](src/clones.rs): `CodeCloneDetector` discovering duplicate logic blocks and `DeadSymbol` instances.
- [`data_flow`](src/data_flow.rs): `DataFlowAnalyzer` tracking `VariableFlow` across sources and sinks.
- [`impact`](src/impact.rs): Evaluates symbol dependency trees to calculate blast-radius percentages.
- [`syntax_validator`](src/syntax_validator.rs): Validates syntactic correctness of proposed patches prior to filesystem writes.

### Programmatic Usage

```rust
use m31a_ast::chunker::AstChunker;

let source = "fn add(a: i32, b: i32) -> i32 { a + b }\nfn sub(a: i32, b: i32) -> i32 { a - b }";
let chunks = AstChunker::chunk_rust_source(source)?;
for chunk in chunks {
    println!("Symbol: {} (lines {}-{})", chunk.name, chunk.start_line, chunk.end_line);
}
```

---

## Testing

```bash
cargo test -p m31a-ast
```
