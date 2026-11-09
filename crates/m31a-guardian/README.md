# `m31a-guardian`

> Out-of-band autonomous security supervisor, prompt injection detector, data exfiltration guard, and sliding-window circuit breaker for M31A.

---

## Why Does This Crate Exist?

When users enable `approval = "approve-for-me"` or run autonomous multi-turn loops (`m31a vibe`), human developers step away from the keyboard. Who watches the agent while it runs autonomously?

`m31a-guardian` acts as an independent, out-of-band AI safety supervisor. Before a mutating tool execution, file deletion, or network request is allowed to commit, Guardian conducts an independent assessment:
- Is this operation consistent with the user's original objective?
- Does this input contain hidden prompt injection payloads attempting to hijack the agent?
- Is the agent attempting to exfiltrate proprietary code or environment secrets over network channels?
- Is the agent installing suspicious or typosquatted packages from public registries?

If an agent behaves erratically or repeatedly violates safety constraints, Guardian trips a sliding-window circuit breaker, immediately halting the turn before any damage occurs.

---

## Core Concepts & How It Works

### 1. Sliding-Window Denial Circuit Breaker (`circuit_breaker.rs`)
If an agent repeatedly attempts blocked operations, it indicates a compromised context or an infinite loop:
- Tracks consecutive denials (`MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN = 3`).
- Tracks recent denials in a sliding window (`AUTO_REVIEW_DENIAL_WINDOW_SIZE = 50`).
- If thresholds are breached, Guardian transitions to `InterruptTurn`, stopping execution and escalating to the user.

### 2. Prompt Injection Detector (`prompt_injection_detector.rs`)
Scans untrusted web search results, issue comments, and downloaded documentation files for prompt injection vectors (e.g., hidden system override instructions, delimiter escapes, role hijacking).

### 3. Data Exfiltration Guard (`data_exfiltration_guard.rs`)
Analyzes outbound network payloads and tool arguments to ensure environment secrets (such as `NVIDIA_API_KEY`, AWS credentials, or private SSH keys) are never transmitted off the machine.

### 4. Package Legitimacy Checker (`package_legitimacy.rs`)
Detects typosquatting and suspicious dependency additions in `Cargo.toml`, `package.json`, and `pyproject.toml`.

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/reviewer.rs`](src/reviewer.rs) | `GuardianReviewer`, `ReviewAssessment` | Coordinates out-of-band safety reviews of pending tool executions. |
| [`src/circuit_breaker.rs`](src/circuit_breaker.rs) | `GuardianRejectionCircuitBreaker`, `GuardianRejectionCircuitBreakerAction` | Tracks consecutive policy denials and halts runaway turns. |
| [`src/prompt_injection_detector.rs`](src/prompt_injection_detector.rs) | `PromptInjectionDetector`, `InjectionScore` | Heuristic and semantic detection of adversarial prompt injections. |
| [`src/data_exfiltration_guard.rs`](src/data_exfiltration_guard.rs) | `DataExfiltrationGuard` | Inspects outgoing traffic for leaked secrets or private files. |
| [`src/package_legitimacy.rs`](src/package_legitimacy.rs) | `PackageLegitimacyChecker` | Detects typosquatting and malicious dependency injection. |

---

## Practical Usage Example

```rust
use m31a_guardian::circuit_breaker::{
    GuardianRejectionCircuitBreaker, GuardianRejectionCircuitBreakerAction,
    GuardianRejectionCircuitBreakerPolicy,
};

fn main() {
    let mut circuit_breaker = GuardianRejectionCircuitBreaker::new();
    let turn_id = "turn_104";

    // Simulate 3 consecutive policy denials for a turn
    for i in 1..=3 {
        let action = circuit_breaker.record_denial(
            turn_id,
            GuardianRejectionCircuitBreakerPolicy::Standard,
        );

        match action {
            GuardianRejectionCircuitBreakerAction::Continue => {
                println!("Denial #{} recorded. Circuit breaker holding.", i);
            }
            GuardianRejectionCircuitBreakerAction::InterruptTurn { consecutive_denials, .. } => {
                println!(
                    "🚨 Circuit breaker TRIPPED after {} consecutive denials! Halting turn.",
                    consecutive_denials
                );
            }
        }
    }
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-guardian
```
