# Artifact Description

**Date:** 26 June 2026

---

## 1. Artifact Overview

| Property | Value |
|----------|-------|
| Name | EFIE (Eshanized File Intelligence Engine) |
| Version | 1.0 (as of commit 73d9965) |
| License | MIT |
| Language | Go 1.24.2 |
| Dependencies | Standard library only |
| Repository | /home/snigdha/Desktop/Helix/M31A |

---

## 2. Artifact Structure

```
M31A/
├── internal/codeintel/efie/          # Core EFIE implementation
│   ├── efie.go                       # Main orchestrator (604 lines)
│   ├── wrapper.go                    # Compatibility layer (152 lines)
│   ├── query.go                      # Query strategies (346 lines)
│   ├── scorer.go                     # Scoring function (232 lines)
│   ├── index.go                      # Index structures (210 lines)
│   ├── bloom.go                      # Bloom filter (108 lines)
│   ├── trie.go                       # Trie index (162 lines)
│   ├── graph.go                      # Graph operations (195 lines)
│   ├── resolve.go                    # Import resolution (243 lines)
│   ├── centrality.go                 # PageRank + betweenness (186 lines)
│   ├── community.go                  # Louvain detection (157 lines)
│   ├── parsers.go                    # Language parsers (452 lines)
│   ├── rng.go                        # Deterministic RNG (24 lines)
│   ├── efie_test.go                  # Unit tests (911 lines)
│   └── efie_extra_test.go            # Additional tests (278 lines)
├── internal/codeintel/               # Original backend
│   ├── codeintel.go                  # Backend switching (379 lines)
│   ├── parser.go                     # File parsers (562 lines)
│   ├── graph.go                      # Graph operations (467 lines)
│   ├── index.go                      # Index structures (160 lines)
│   └── relevance.go                  # Scoring (238 lines)
├── paper/                            # JOSS manuscript
│   ├── paper.md                      # Main paper (1337 words)
│   ├── paper.bib                     # Bibliography
│   └── evaluate/                     # Evaluation framework
│       ├── generate.go               # Synthetic repo generator
│       ├── benchmark_test.go         # Go benchmarks
│       ├── evaluate.go               # Main evaluation
│       ├── helpers.go                # Benchmark helpers
│       └── types.go                  # Data structures
├── EFIE/                             # Algorithm documentation
│   ├── EFIE_ALGORITHM_V2.md          # Algorithm specification
│   ├── EFIE_RESEARCH.md              # Research paper
│   ├── EFIE_PATENT_WHITE_PAPER.md    # Patent claims
│   └── BENCHMARK_REPORT.md           # Existing benchmarks
└── research/                         # Research validation (this directory)
    ├── ImplementationAudit.md        # Phase 1 deliverable
    ├── ClaimInventory.json           # Phase 2 deliverable
    ├── ImplementationMismatch.md     # Phase 3 deliverable
    ├── ExperimentalPlan.md           # Phase 4 deliverable
    ├── ThreatsToValidity.md          # Threat analysis
    ├── PeerReview.md                 # Simulated review
    ├── ArtifactDescription.md        # This file
    ├── ReproducibilityGuide.md       # How to reproduce
    └── JOSSChecklist.md              # JOSS compliance
```

---

## 3. Build Instructions

```bash
# Clone repository
git clone <repository-url>
cd M31A

# Build
go build ./...

# Run unit tests
go test ./internal/codeintel/efie/ -v

# Run benchmarks
go test ./paper/evaluate/ -bench=. -benchmem -count=30

# Run evaluation
go run paper/evaluate/evaluate.go -repos /path/to/repos -runs 30
```

---

## 4. Hardware Requirements

| Component | Minimum | Recommended |
|-----------|---------|------------|
| CPU | 4 cores | 8+ cores |
| RAM | 8 GB | 16+ GB |
| Disk | 10 GB free | SSD recommended |
| OS | Linux, macOS, Windows | Linux |

---

## 5. Software Requirements

| Software | Version | Purpose |
|----------|---------|---------|
| Go | 1.24.2+ | Build and test |
| Git | 2.0+ | Source control |
| Docker | 20.10+ | Reproducible environment (optional) |

---

## 6. Expected Results

### 6.1 Build Performance
- EFIE build time: 100ms-6s depending on repository size
- Original build time: 50ms-300ms
- EFIE/Original ratio: 2-20x slower

### 6.2 Query Performance
- EFIE query time: 0.1-30ms
- Original query time: 0.001-0.3ms
- EFIE/Original ratio: 10-100x slower

### 6.3 Test Results
- 51/66 tests pass
- 15 tests skipped (require full build)
- 0 tests fail

---

## 7. Known Issues

| # | Issue | Impact | Workaround |
|---|-------|--------|-----------|
| 1 | PageRank computes wrong direction | Centrality measures wrong thing | Fix centrality.go |
| 2 | Import proximity unbounded | Scoring weights meaningless | Cap at 20.0 |
| 3 | Incremental build not implemented | 63% faster rebuild claim theoretical | Implement or remove claim |
| 4 | Convergence not monitored | 4% guarantee unverified | Add convergence check |
| 5 | Synthetic repos produce 0 communities | Community claims untestable | Use real repos |
