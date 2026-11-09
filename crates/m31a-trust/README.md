# `m31a-trust`

> Workspace trust boundary enforcement, Git repository security auditing, and Git hook execution armor for M31A.

---

## Why Does This Crate Exist?

When developers clone an unfamiliar or public open-source repository and run an AI coding assistant, they expose themselves to repository-borne code execution attacks. Malicious repositories can conceal arbitrary execution triggers inside:
- Executable `.git/hooks/*` (e.g., `pre-commit`, `post-checkout`).
- Overridden `core.hooksPath` configurations pointing to hidden payload directories.
- Malicious `.git/config` aliases invoking shell commands (`git checkout` executing hidden subshells).
- Weaponized `.gitmodules` URLs targeting CVE vulnerabilities.

`m31a-trust` establishes an explicit workspace trust boundary:
- Before executing commands in a new directory, it runs `GitArmor` to audit Git configurations and hooks.
- Maintains a cryptographic trust database of approved repositories (`trust_store.rs`).
- Shields untrusted repositories until the user explicitly permits execution via `m31a trust allow`.

---

## Core Concepts & How It Works

### 1. Git Armor (`git_armor.rs`)
Inspects the repository structure before granting tool execution permissions:
- Scans `.git/hooks` for active executable scripts.
- Checks `.git/config` and included configuration files for custom `core.hooksPath` and dangerous aliases.
- Generates a `GitArmorReport` with a clear verdict (`Safe`, `Caution`, or `Dangerous`).

### 2. Workspace Trust Store (`trust_store.rs`)
Stores approved workspace paths and repository hashes in `~/.m31a/trust.db`. If a repository's remote URL or root path changes, trust is invalidated until re-confirmed.

### 3. Capability Gating (`capability.rs`)
Untrusted repositories operate with restricted tool privileges (read-only file inspection only; shell execution and patch mutations are blocked).

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/git_armor.rs`](src/git_armor.rs) | `GitArmor`, `GitArmorReport`, `GitRisk`, `ArmorVerdict` | Scans `.git` metadata, hooks, and configs for exploit vectors. |
| [`src/trust_store.rs`](src/trust_store.rs) | `TrustStore`, `TrustRecord`, `TrustLevel` | Persistent database of trusted repository paths and permissions. |
| [`src/capability.rs`](src/capability.rs) | `TrustCapabilityFilter` | Enforces restricted toolsets in untrusted workspaces. |

---

## Practical Usage Example

```rust
use m31a_trust::git_armor::{ArmorVerdict, GitArmor};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let repo_path = Path::new(".");
    let report = GitArmor::inspect_repository(repo_path)?;

    println!("Repository Trust Verdict: {:?}", report.verdict);
    println!("Hooks Detected: {}", report.hooks_detected);

    match report.verdict {
        ArmorVerdict::Safe => {
            println!("Repository verified clean. Unrestricted tools permitted.");
        }
        ArmorVerdict::Caution { high_risks, .. } => {
            println!("Caution: {} high-risk vectors detected.", high_risks);
        }
        ArmorVerdict::Dangerous { critical_risks } => {
            eprintln!("Dangerous repository! {} critical exploit vectors found.", critical_risks);
            for risk in report.risks {
                eprintln!("- [{:?}] {}: {}", risk.severity, risk.title, risk.description);
            }
        }
    }

    Ok(())
}
```

---

## CLI Integration

```bash
# Check trust status of current workspace
m31a trust

# Grant full trust to current repository
m31a trust allow
```

---

## Testing & Verification

```bash
cargo test -p m31a-trust
```
