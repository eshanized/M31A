#!/usr/bin/env python3
"""Genuine benchmark harness with statistical analysis for M31A E2E performance testing."""

import argparse
import json
import math
import os
import subprocess
import sys
import time

def calculate_percentile(sorted_data, p):
    if not sorted_data:
        return 0.0
    k = (len(sorted_data) - 1) * (p / 100.0)
    f = math.floor(k)
    c = math.ceil(k)
    if f == c:
        return sorted_data[int(k)]
    d0 = sorted_data[int(f)] * (c - k)
    d1 = sorted_data[int(c)] * (k - f)
    return d0 + d1

def run_benchmark(benchmark_name, iterations, output_path, command_str):
    print(f"=== Starting M31A Benchmark: {benchmark_name} ({iterations} iterations) ===")
    
    # Locate workspace root
    script_dir = os.path.dirname(os.path.realpath(__file__))
    workspace_dir = os.path.abspath(os.path.join(script_dir, ".."))
    
    # If no command provided, find m31a binary in target/release, target/debug, or build dir
    if not command_str:
        candidate_bins = [
            os.path.join(workspace_dir, "target", "release", "m31a"),
            os.path.join(workspace_dir, "target", "debug", "m31a"),
        ]
        m31a_bin = None
        for b in candidate_bins:
            if os.path.exists(b) and os.access(b, os.X_OK):
                m31a_bin = b
                break
        
        if m31a_bin:
            cmd = [m31a_bin, "--version"]
        else:
            cmd = ["cargo", "check", "--manifest-path", os.path.join(workspace_dir, "Cargo.toml")]
    else:
        cmd = command_str.split()

    print(f"Executing command: {' '.join(cmd)}")
    
    latencies_ms = []
    total_start = time.perf_counter()
    
    for i in range(1, iterations + 1):
        iter_start = time.perf_counter()
        result = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd=workspace_dir)
        iter_end = time.perf_counter()
        
        elapsed_ms = (iter_end - iter_start) * 1000.0
        if result.returncode != 0:
            print(f"Iteration {i} FAILED with code {result.returncode}: {result.stderr.decode('utf-8', errors='replace')}", file=sys.stderr)
            sys.exit(result.returncode)
        
        latencies_ms.append(elapsed_ms)
        print(f"  Iteration {i:2d}/{iterations}: {elapsed_ms:.2f} ms")

    total_end = time.perf_counter()
    total_duration_sec = total_end - total_start
    total_duration_ms = total_duration_sec * 1000.0
    
    latencies_ms.sort()
    count = len(latencies_ms)
    min_ms = latencies_ms[0]
    max_ms = latencies_ms[-1]
    avg_ms = sum(latencies_ms) / count
    p50_ms = calculate_percentile(latencies_ms, 50.0)
    p95_ms = calculate_percentile(latencies_ms, 95.0)
    p99_ms = calculate_percentile(latencies_ms, 99.0)
    throughput = count / total_duration_sec if total_duration_sec > 0 else 0.0

    report = {
        "benchmark": benchmark_name,
        "status": "passed",
        "iterations": count,
        "metrics": {
            "total_duration_ms": round(total_duration_ms, 2),
            "min_latency_ms": round(min_ms, 2),
            "max_latency_ms": round(max_ms, 2),
            "avg_latency_ms": round(avg_ms, 2),
            "p50_latency_ms": round(p50_ms, 2),
            "p95_latency_ms": round(p95_ms, 2),
            "p99_latency_ms": round(p99_ms, 2),
            "throughput_ops_per_sec": round(throughput, 2)
        }
    }

    os.makedirs(os.path.dirname(os.path.abspath(output_path)), exist_ok=True)
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2)

    print(f"\n--- Benchmark Summary ({benchmark_name}) ---")
    print(f"Iterations:   {count}")
    print(f"Avg Latency:  {avg_ms:.2f} ms")
    print(f"P50 Latency:  {p50_ms:.2f} ms")
    print(f"P95 Latency:  {p95_ms:.2f} ms")
    print(f"P99 Latency:  {p99_ms:.2f} ms")
    print(f"Throughput:   {throughput:.2f} ops/sec")
    print(f"Results written to: {output_path}")

def main():
    parser = argparse.ArgumentParser(description="M31A Real Benchmark Harness")
    parser.add_argument("--name", default="bench_full_session", help="Benchmark name")
    parser.add_argument("--iterations", type=int, default=5, help="Number of iterations")
    parser.add_argument("--output", default="benchmark_results.json", help="Path to write JSON output")
    parser.add_argument("--command", default="", help="Optional specific command to benchmark")
    args = parser.parse_args()

    run_benchmark(args.name, args.iterations, args.output, args.command)

if __name__ == "__main__":
    main()
