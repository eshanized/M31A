# `m31a-guardian`

Defensive security guard, prompt injection detector, risk evaluator, and circuit breaker for **M31A**.

---

## Overview

`m31a-guardian` operates as an out-of-band adversarial security supervisor. It evaluates model actions, detects prompt injection and data exfiltration patterns, checks package legitimacy (combating typosquatting attacks in `npm` and `cargo`), and triggers a sliding-window circuit breaker if anomalous destructive patterns occur.

---

## User-Facing Capabilities

- **Automatic Supervisor Approval (`--approve-for-me`)**: Routes mutating actions through the Guardian supervisor, allowing fast autonomous execution while halting risky operations.
- **Circuit Breaker Protection**: Halts the session immediately if repeated policy violations occur, preventing runaway loops from damaging code or leaking data.
- **Typosquatting Detection**: Alerts users if an agent attempts to install unverified packages with suspicious names.

---

## Developer Guide & Architecture

### Key Modules

- [`circuit_breaker`](src/circuit_breaker.rs): `CircuitBreaker` enforcing sliding-window limits (`AUTO_REVIEW_DENIAL_WINDOW_SIZE = 5`, `MAX_DENIALS_BEFORE_TRIP = 3`).
- [`prompt_injection_detector`](src/prompt_injection_detector.rs): Scans incoming prompts and tool outputs for adversarial jailbreak and prompt override patterns.
- [`package_legitimacy`](src/package_legitimacy.rs): `PackageLegitimacyChecker` evaluating package signals across `Ecosystem::Cargo`, `Ecosystem::Npm`, and `Ecosystem::Pypi`.
- [`data_exfiltration_guard`](src/data_exfiltration_guard.rs): Prevents secret tokens or private keys from appearing in outbound request bodies.

---

## Testing

```bash
cargo test -p m31a-guardian
```
