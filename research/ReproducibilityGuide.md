# Reproducibility Guide

**Date:** 26 June 2026

---

## 1. Quick Start

```bash
# Clone and build
git clone <repository-url>
cd M31A
go build ./...

# Run all tests
go test ./internal/codeintel/efie/ -v

# Run evaluation (requires 10-30 minutes)
go test ./paper/evaluate/ -bench=. -benchmem -count=30
```

---

## 2. Environment Setup

### 2.1 Go Installation
```bash
# Install Go 1.24.2
wget https://go.dev/dl/go1.24.2.linux-amd64.tar.gz
sudo tar -C /usr/local -xzf go1.24.2.linux-amd64.tar.gz
export PATH=$PATH:/usr/local/go/bin

# Verify
go version  # Should output: go version go1.24.2 linux/amd64
```

### 2.2 Docker (Optional)
```bash
# Build reproducible environment
docker build -t efie-eval .

# Run evaluation
docker run -v $(pwd)/results:/results efie-eval
```

---

## 3. Reproducing Key Results

### 3.1 Build Time Benchmark (Table 1)

```bash
# Generate synthetic repositories
go run paper/evaluate/generate.go -sizes 100,500,1000,2000,5000,10000 -output /tmp/repos

# Run build benchmarks (30 runs each)
go test ./paper/evaluate/ -run=^$ -bench=BenchmarkBuild -benchmem -count=30 -repos=/tmp/repos

# Expected: EFIE ~5.7s for 10K files, Original ~0.3s
```

### 3.2 Query Time Benchmark (Table 2)

```bash
# Run query benchmarks (30 runs each)
go test ./paper/evaluate/ -run=^$ -bench=BenchmarkQuery -benchmem -count=30 -repos=/tmp/repos

# Expected: EFIE ~30ms, Original ~0.6ms
```

### 3.3 Scalability Analysis (Figure 1)

```bash
# Generate scalability data
go test ./paper/evaluate/ -run=^$ -bench=BenchmarkBuild -benchmem -count=30 -repos=/tmp/repos -output=results/scalability.json

# Generate plot
python3 scripts/plot_scalability.py results/scalability.json results/scalability.png

# Expected: Linear scaling, R²=0.97 for build, R²=0.99 for query
```

### 3.4 Unit Tests (Table 3)

```bash
# Run all tests
go test ./internal/codeintel/efie/ -v -count=1

# Expected: 51 pass, 15 skipped, 0 fail
```

---

## 4. Statistical Analysis

### 4.1 Computing Confidence Intervals

```python
import numpy as np
from scipy import stats

def compute_ci(data, confidence=0.95):
    n = len(data)
    mean = np.mean(data)
    se = stats.sem(data)
    ci = se * stats.t.ppf((1 + confidence) / 2, n - 1)
    return mean, ci

# Example
build_times = [5679.2, 5502.7, 5855.6, ...]  # 30 measurements
mean, ci = compute_ci(build_times)
print(f"Build time: {mean:.1f} ± {ci:.1f} ms (95% CI)")
```

### 4.2 Paired t-test

```python
from scipy import stats

def paired_ttest(efie_times, original_times):
    t_stat, p_value = stats.ttest_rel(efie_times, original_times)
    return t_stat, p_value

# Example
t, p = paired_ttest(efie_build, original_build)
print(f"t={t:.2f}, p={p:.4f}")
```

---

## 5. Troubleshooting

### 5.1 Build Fails
```bash
# Check Go version
go version

# Clean build cache
go clean -cache

# Try again
go build ./...
```

### 5.2 Tests Fail
```bash
# Run with verbose output
go test ./internal/codeintel/efie/ -v

# Run specific test
go test ./internal/codeintel/efie/ -run=TestTrieSearch -v
```

### 5.3 Benchmarks Timeout
```bash
# Increase timeout
go test ./paper/evaluate/ -bench=. -benchmem -timeout=30m

# Run single benchmark
go test ./paper/evaluate/ -bench=BenchmarkBuild10000 -benchmem
```

---

## 6. File Checksums

After building, verify these checksums:

```bash
# Build binary
go build -o efie-eval ./paper/evaluate/

# Compute checksum
sha256sum efie-eval

# Expected: <checksum will be added after final build>
```

---

## 7. Contact

For questions about reproducing results:
- Author: Eshan Roy <eshanized@proton.me>
- ORCID: 0009-0007-1261-6805
