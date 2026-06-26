# Contributing to EGINE

Thank you for your interest in contributing to EGINE! This document provides guidelines and information for contributors.

## Code of Conduct

Please be respectful and constructive in all interactions. We are committed to providing a welcoming and inclusive experience for everyone.

## How to Contribute

### Reporting Bugs

1. Check existing issues to avoid duplicates
2. Open a new issue with:
   - Clear title and description
   - Steps to reproduce
   - Expected vs actual behavior
   - Go version and OS information
   - Minimal code example if applicable

### Suggesting Features

1. Open an issue with the `enhancement` label
2. Describe the use case and expected behavior
3. Include mathematical foundations if applicable

### Submitting Changes

1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes following the style guidelines
4. Add or update tests
5. Update documentation if needed
6. Submit a pull request

## Development Setup

### Prerequisites

- Go 1.25+
- Git
- Make (optional, for convenience targets)

### Getting Started

```bash
git clone https://github.com/eshanized/M31A.git
cd M31A
make dev  # Install development dependencies
make test # Run tests
```

### Project Structure

```
EGINE/
├── graph/          # Core graph algorithms
│   ├── pagerank.go
│   ├── community.go
│   ├── centrality.go
│   └── ...
├── index/          # Trie and indexing
├── bloom/          # Bloom filter
├── examples/       # Usage examples
└── paper.bib       # JOSS citations
```

## Style Guidelines

### Code Style

- Follow standard Go conventions (`gofmt`, `go vet`)
- Use meaningful variable and function names
- Keep functions focused and concise
- Document exported functions and types

### Mathematical Notation

- Use LaTeX notation in comments for mathematical algorithms
- Include complexity analysis for new algorithms
- Reference original papers where applicable

### Testing

- Write table-driven tests
- Include benchmark tests for performance-critical code
- Aim for >90% code coverage

```go
func TestPageRank(t *testing.T) {
    tests := []struct {
        name     string
        graph    *DirectedGraph
        damping  float64
        expected map[string]float64
    }{
        {
            name:    "simple graph",
            graph:   createTestGraph(),
            damping: 0.85,
            expected: map[string]float64{
                "A": 0.35,
                "B": 0.35,
                "C": 0.30,
            },
        },
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            result := PageRank(tt.graph, tt.damping, 20)
            for node, expected := range tt.expected {
                if diff := math.Abs(result[node] - expected); diff > 0.01 {
                    t.Errorf("Node %s: got %f, want %f", node, result[node], expected)
                }
            }
        })
    }
}
```

## Commit Messages

- Use present tense ("Add feature" not "Added feature")
- Keep first line under 72 characters
- Reference issue numbers when applicable
- Include mathematical proofs for algorithm changes

Example:
```
Add approximate betweenness centrality

Implements Brandes' algorithm with vertex sampling for O(V·m)
time complexity. Includes mathematical proof of error bounds.

Fixes #42
```

## Pull Request Process

1. Ensure all tests pass
2. Update documentation for new features
3. Add yourself to Contributors section if first contribution
4. Request review from maintainers
5. Address feedback promptly

## Algorithms and Mathematics

EGINE is fundamentally a mathematical library. When contributing algorithms:

1. **Cite sources**: Reference original papers in `paper.bib`
2. **Provide proofs**: Include correctness proofs in comments
3. **Analyze complexity**: Document time and space complexity
4. **Validate empirically**: Include benchmarks against baselines

## Documentation

- Update README.md for new features
- Add examples for new functionality
- Keep INSTALL.md current with dependencies
- Update JOSS paper (EGINE_JOSS.md) for significant additions

## Questions?

Open an issue with the `question` label or start a discussion in the repository's Discussions tab.

Thank you for contributing to EGINE!
