# `m31a-execpolicy`

> Fine-grained command prefix rules engine, AST command decomposition, network destination filtering, and security presets for M31A.

---

## Why Does This Crate Exist?

Even inside a sandbox container, certain operations should not be permitted without user oversight:
- Running destructive commands like `git reset --hard` or `git clean -fd`.
- Initiating outbound network connections to unapproved external endpoints.
- Chaining dangerous shell operations via pipes, subshells, or redirection (`curl ... | bash`).

`m31a-execpolicy` acts as the execution policy gatekeeper. Before any shell tool call or network socket is spawned, it inspects the command tokens, decomposes compound shell commands via AST parsing, checks network destinations against whitelist/blacklist rules, and renders a definitive verdict: `Allow`, `Prompt`, or `Deny`.

---

## Core Concepts & How It Works

### 1. Prefix Rules with Alternative Tokens (`PatternToken`)
Rules are expressed as pattern sequences matching command invocations:
- `PatternToken::Single("cargo")`: Matches the exact binary.
- `PatternToken::Alts(vec!["check", "test", "build"])`: Matches any alternative token.
- Wildcards (`*`): Matches any token.

Example Rule:
`cargo [check|test|build] *` $\to$ `Decision::Allow`  
`rm -rf *` $\to$ `Decision::Prompt`  
`curl * | sh` $\to$ `Decision::Deny`

### 2. AST Command Decomposition (`ast_decomposer.rs`)
Attackers or complex scripts often chain commands: `cargo check && rm -rf dist/ || curl malicious.site`.  
`m31a-execpolicy` uses AST parsing to break compound shell pipelines into individual atomic sub-commands and verifies each segment independently.

### 3. Network Connect Policy (`connect_policy.rs`)
Restricts agent outbound connections by host, port, and protocol, defending against DNS rebinding and SSRF attacks.

### 4. Security Presets (`presets.rs`)
Provides pre-configured security profiles:
- `ReadOnly`: Forbids all mutating commands.
- `Standard`: Whitelists common build tools (`cargo`, `npm`, `python`); prompts on destructive actions.
- `Permissive`: Minimizes prompts while retaining sandbox isolation.
- `Airgapped`: Strictly disables all network communication.

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/policy.rs`](src/policy.rs) | `ExecPolicy`, `PrefixRule`, `PatternToken`, `Decision` | Core rule matching and decision evaluation (`Allow`, `Prompt`, `Deny`). |
| [`src/ast_decomposer.rs`](src/ast_decomposer.rs) | `AstDecomposer`, `CommandSegment` | Breaks piped, chained, and subshell commands into atomic units. |
| [`src/connect_policy.rs`](src/connect_policy.rs) | `ConnectPolicy`, `NetworkDestination` | Filters host, IP, and port destinations. |
| [`src/presets.rs`](src/presets.rs) | `SecurityPreset`, `PresetRegistry` | Predefined security configurations (`Standard`, `Airgapped`, etc.). |

---

## Practical Usage Example

```rust
use m31a_execpolicy::policy::{Decision, ExecPolicy, PrefixRule};

fn main() -> anyhow::Result<()> {
    let mut policy = ExecPolicy::default();

    // 1. Add rule allowing cargo test
    policy.add_rule(PrefixRule::from_pattern(
        "cargo test",
        Decision::Allow,
        Some("Permit running test suites".into()),
    )?);

    // 2. Add rule prompting for rm commands
    policy.add_rule(PrefixRule::from_pattern(
        "rm *",
        Decision::Prompt,
        Some("Requires human review before file deletion".into()),
    )?);

    // 3. Evaluate a proposed command
    let check_decision = policy.evaluate("cargo test --workspace");
    println!("Evaluation for 'cargo test': {:?}", check_decision);
    assert_eq!(check_decision, Decision::Allow);

    let rm_decision = policy.evaluate("rm -rf target/");
    println!("Evaluation for 'rm -rf': {:?}", rm_decision);
    assert_eq!(rm_decision, Decision::Prompt);

    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-execpolicy
```
