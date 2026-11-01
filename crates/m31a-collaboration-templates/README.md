# `m31a-collaboration-templates`

Pre-configured workflow templates for pair programming, code review, debugging, and architecture design in **M31A**.

---

## Overview

`m31a-collaboration-templates` provides structured session templates that initialize system prompts, security presets, tool allowances, and execution step pipelines tailored to specific software engineering workflows.

---

## User-Facing Capabilities

- **Workflow Presets (`m31a template apply <template-id>`)**: Instantly switches agent configuration to pre-tested configurations for:
  - `pair-programming`: Interactive coding with human-in-the-loop approvals.
  - `rigorous-code-review`: Read-only security audit and blast-radius evaluation.
  - `system-architecture`: Exploration, threat modeling, and design document drafting.
  - `zero-babysitter-debug`: Deep root-cause isolation and test suite regression checks.

---

## Developer Guide & Architecture

### Key Types

- `CollaborationTemplate`: Root struct containing template ID, description, category, tags, and step pipeline.
- `TemplateConfig`: Bundles `SandboxPolicy`, `ApprovalPolicy`, and model temperature recommendations.
- `TemplateStep`: Ordered steps with specialized instructions injected sequentially into agent context.

---

## Testing

```bash
cargo test -p m31a-collaboration-templates
```
