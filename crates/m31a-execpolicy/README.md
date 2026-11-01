# `m31a-execpolicy`

Execution policies, command AST decomposition, egress firewall, and security decision engine for **M31A**.

---

## Overview

`m31a-execpolicy` evaluates all commands, scripts, and outbound socket connections against security policies before they are permitted to run. It decomposes shell command ASTs, matches tokens against alternative patterns (`PatternToken::Alts`), enforces DNS rebinding protections, and applies predefined security presets.

---

## User-Facing Capabilities

- **Security Presets (`--preset <name>`)**: Pre-configured security profiles (`read-only`, `standard`, `permissive`, `airgapped`, `danger-full-access`).
- **Command Vetting**: Blocks dangerous commands (`rm -rf /`, fork bombs, unauthorized sudo) and prompts for intermediate actions (`npm install`, `git push`).
- **Egress Firewall**: Restricts outbound connections to authorized developer registries while preventing SSRF against internal LAN services.

---

## Developer Guide & Architecture

### Key Modules

- [`policy`](src/policy.rs): `ExecPolicy` engine evaluating command vectors and returning tri-state decisions (`allow`, `prompt`, `deny`).
- [`ast_decomposer`](src/ast_decomposer.rs): `ShellAstDecomposer` breaking complex piped shell pipelines into constituent command invocations.
- [`domain_matcher`](src/domain_matcher.rs): Hostname and CIDR pattern matching for network filtering.
- [`dns_rebinding`](src/dns_rebinding.rs): Defends against DNS rebinding attacks targeting localhost or cloud metadata services (`169.254.169.254`).
- [`presets`](src/presets.rs): Built-in security presets and policy factories.

---

## Testing

```bash
cargo test -p m31a-execpolicy
```
