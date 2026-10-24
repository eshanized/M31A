---
kind: config
schema_version: 1
data:
  agents:
    default_model: meta/llama-3.2-11b-vision-instruct
    fast_auxiliary_model: meta/llama-3.2-11b-vision-instruct
    max_tokens: 8192
  git:
    auto_commit: true
    branch_prefix: m31a/mission
    enabled: true
    execution_isolation: best_effort
    push_policy: ask
    retention_policy: keep_on_failure
  policy:
    default_action: ask
    denied_tools: []
    interactive_approvals: true
  profile: balanced
  provider:
    default: nvidia_nim
    nvidia_nim:
      base_url: https://integrate.api.nvidia.com/v1
      default_model: meta/llama-3.2-11b-vision-instruct
  runtime:
    concurrency_limit: 4
    sandbox_mode: standard
    timeout_secs: 300
  tui:
    compact_mode: false
    fps: 30
    theme: dark-slate-cyan
  workflow:
    ambiguity_threshold_percent: 15
    auto_advance: false
    max_discovery_turns: 4
    projection_dir: .m31a/project
    require_architecture_approval: true
    require_charter_approval: true
    require_roadmap_approval: true
    research: true
    research_concurrency: 4
---

# M31A Configuration

Authoritative configuration values are stored in the YAML front matter above.
