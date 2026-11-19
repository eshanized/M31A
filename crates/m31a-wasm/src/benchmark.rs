//! WebAssembly Runtime Performance Benchmarking.
//!
//! Measures module instantiation latency, function dispatch overhead,
//! fuel consumption rates, and memory limits under high-throughput invocation.

use super::manifest::{WasmPluginManifest, WasmResourceLimits};
use super::runtime::WasmRuntime;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Benchmark performance metrics report for WebAssembly execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmBenchmarkReport {
    pub total_invocations: usize,
    pub instantiation_millis: f64,
    pub total_execution_millis: f64,
    pub throughput_calls_per_sec: f64,
    pub mean_latency_micros_per_call: f64,
    pub fuel_consumed_per_call: u64,
}

pub struct WasmBenchmarker;

impl WasmBenchmarker {
    /// Compiles a simple deterministic WebAssembly WAT module for benchmarking.
    pub fn echo_wat_module() -> &'static str {
        r#"
(module
  (memory (export "memory") 1)
  (func (export "m31a_tool_call") (param $name_ptr i32) (param $name_len i32) (param $args_ptr i32) (param $args_len i32) (result i32)
    ;; Copy 5 bytes "pong\0" to offset 4096
    (i32.store8 (i32.const 4096) (i32.const 112)) ;; 'p'
    (i32.store8 (i32.const 4097) (i32.const 111)) ;; 'o'
    (i32.store8 (i32.const 4098) (i32.const 110)) ;; 'n'
    (i32.store8 (i32.const 4099) (i32.const 103)) ;; 'g'
    (i32.const 4) ;; Return 4 bytes
  )
)
"#
    }

    /// Runs a reproducible benchmark across `invocations`.
    pub fn run_benchmark(invocations: usize) -> WasmBenchmarkReport {
        let runtime = WasmRuntime::new();
        let wat = Self::echo_wat_module();
        let wasm_bytes = wat::parse_str(wat).expect("Valid WAT module");

        let manifest = WasmPluginManifest {
            name: "bench-echo".to_string(),
            version: "1.0.0".to_string(),
            abi_version: 1,
            description: "Benchmark echo plugin".to_string(),
            author: None,
            capabilities: Vec::new(),
            limits: WasmResourceLimits {
                max_memory_pages: 10,
                max_fuel: 10_000_000,
                execution_deadline_ms: 5000,
            },
        };

        let inst_start = Instant::now();
        let mut instance = runtime
            .load_plugin(manifest, &wasm_bytes, std::env::temp_dir(), "{}")
            .expect("Plugin must load");
        let instantiation_millis = inst_start.elapsed().as_secs_f64() * 1000.0;

        let initial_fuel = instance.remaining_fuel().unwrap_or(0);

        let exec_start = Instant::now();
        for _ in 0..invocations {
            let res = instance.call_tool("ping", "{}").expect("Call succeeded");
            assert_eq!(res, "pong");
        }
        let total_execution = exec_start.elapsed();
        let total_execution_millis = total_execution.as_secs_f64() * 1000.0;

        let final_fuel = instance.remaining_fuel().unwrap_or(0);
        let total_fuel = initial_fuel.saturating_sub(final_fuel);
        let fuel_per_call = if invocations > 0 {
            total_fuel / (invocations as u64)
        } else {
            0
        };

        let throughput = if total_execution.as_secs_f64() > 0.0 {
            invocations as f64 / total_execution.as_secs_f64()
        } else {
            0.0
        };

        let mean_micros = if invocations > 0 {
            (total_execution.as_micros() as f64) / (invocations as f64)
        } else {
            0.0
        };

        WasmBenchmarkReport {
            total_invocations: invocations,
            instantiation_millis,
            total_execution_millis,
            throughput_calls_per_sec: throughput,
            mean_latency_micros_per_call: mean_micros,
            fuel_consumed_per_call: fuel_per_call,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_benchmark_runs() {
        let report = WasmBenchmarker::run_benchmark(100);
        assert_eq!(report.total_invocations, 100);
        assert!(report.throughput_calls_per_sec > 10.0);
        assert!(report.fuel_consumed_per_call > 0);
    }
}
