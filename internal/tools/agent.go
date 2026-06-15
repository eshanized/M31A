package tools

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tools/subagent"
	"github.com/eshanized/M31A/internal/types"
)

// Compile-time interface check
var _ types.Tool = (*Agent)(nil)

// Agent is a tool that spawns a parallel subagent to perform an independent
// task. The parent conversation receives the subagent's ID immediately and
// can continue issuing other tool calls while the subagent runs.
//
// Background mode (default) returns the agent ID and delivers the final
// summary to the parent via SubagentEvent messages consumed by the TUI.
// Foreground mode (run_in_background=false) blocks until the subagent
// finishes and returns its summary as this tool's output.
type Agent struct {
	manager *subagent.Manager
	// isChild prevents recursive fan-out: a subagent's own dispatcher
	// registers an Agent with isChild=true, which forces foreground mode.
	isChild bool
}

// NewAgent creates an Agent tool backed by the given manager.
func NewAgent(m *subagent.Manager, isChild bool) *Agent {
	return &Agent{manager: m, isChild: isChild}
}

func (t *Agent) Name() string               { return "Agent" }
func (t *Agent) RiskLevel() types.RiskLevel { return types.RiskSafe }

func (t *Agent) Description() string {
	return `Spawn a parallel subagent to perform an independent task.

Use this when:
- You need to explore multiple code paths in parallel (grep + read + bash).
- You want to delegate a well-scoped investigation to a focused child agent.
- The task can run without user interaction.

Parameters:
- description (required, 3-5 words): short task label shown in the TUI.
- prompt (required): full task description with concrete objectives.
- name (optional): stable label for the subagent (e.g., "tui-explorer").
- isolation: "worktree" (own git worktree, default) or "default" (shared dir).
- run_in_background: true (default, async) or false (block until done).

Each subagent runs in its own git worktree with full tool access. Max 8 run
concurrently. Do not spawn more than needed — each subagent costs tokens.`
}

// ParameterSchema returns the JSON schema exposed to the LLM.
func (t *Agent) ParameterSchema() string {
	return `{
  "type": "object",
  "properties": {
    "description": {"type": "string", "description": "Short 3-5 word task label"},
    "prompt":      {"type": "string", "description": "Full task description"},
    "name":        {"type": "string", "description": "Optional stable label"},
    "isolation":   {"type": "string", "enum": ["worktree","default"], "default": "worktree"},
    "run_in_background": {"type": "boolean", "default": true}
  },
  "required": ["description", "prompt"]
}`
}

// Execute validates input, spawns the subagent, and returns its ID + status.
func (t *Agent) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	if t.manager == nil {
		return types.ToolResult{Error: "Agent tool: subagent manager not configured"}, nil
	}

	var req struct {
		Description     string `json:"description"`
		Prompt          string `json:"prompt"`
		Name            string `json:"name"`
		Isolation       string `json:"isolation"`
		RunInBackground *bool  `json:"run_in_background"`
	}
	raw, err := json.Marshal(input.Params)
	if err != nil {
		return types.ToolResult{Error: "Agent: invalid params: " + err.Error()}, nil
	}
	if unmarshalErr := json.Unmarshal(raw, &req); unmarshalErr != nil {
		return types.ToolResult{Error: "Agent: invalid params: " + unmarshalErr.Error()}, nil
	}
	if req.Description == "" {
		return types.ToolResult{Error: "Agent: 'description' is required"}, nil
	}
	if req.Prompt == "" {
		return types.ToolResult{Error: "Agent: 'prompt' is required"}, nil
	}

	var iso subagent.Isolation
	switch req.Isolation {
	case "", "worktree":
		iso = subagent.IsolationWorktree
	case "default":
		iso = subagent.IsolationDefault
	default:
		return types.ToolResult{Error: fmt.Sprintf("Agent: unknown isolation %q", req.Isolation)}, nil
	}

	background := true
	if req.RunInBackground != nil {
		background = *req.RunInBackground
	}
	// Recursive spawns are forced foreground to prevent runaway fan-out.
	if t.isChild {
		background = false
	}

	spawnReq := subagent.SpawnRequest{
		Description: req.Description,
		Prompt:      req.Prompt,
		Name:        req.Name,
		Isolation:   iso,
		Background:  background,
	}

	id, sa, err := t.manager.Spawn(ctx, spawnReq)
	if err != nil {
		return types.ToolResult{Error: "Agent spawn failed: " + err.Error()}, nil
	}

	// Build the response payload.
	resp := map[string]any{
		"agent_id": id,
		"status":   string(sa.Info.Status),
		"worktree": sa.Info.Worktree,
	}
	if background {
		resp["mode"] = "background"
		resp["message"] = "Subagent spawned. Its progress will appear in the TUI subagent panel and its summary will be delivered when it finishes."
	} else {
		resp["mode"] = "foreground"
		resp["summary"] = sa.Info.LastSummary
		resp["tool_calls"] = sa.Info.ToolCalls
		resp["input_tokens"] = sa.Info.InputToks
		resp["output_tokens"] = sa.Info.OutputToks
		if sa.Info.LastError != "" {
			resp["error"] = sa.Info.LastError
		}
	}
	out, _ := json.Marshal(resp)
	return types.ToolResult{Output: string(out)}, nil
}

// dispatcherAdapter wraps a *Dispatcher so it satisfies the
// subagent.ToolDispatcher interface without introducing an import cycle.
type dispatcherAdapter struct {
	d *Dispatcher
}

// Compile-time interface check
var _ subagent.ToolDispatcher = (*dispatcherAdapter)(nil)

func (a *dispatcherAdapter) Execute(ctx context.Context, call subagent.ToolCallInput) (subagent.ToolCallOutput, error) {
	res, err := a.d.Execute(ctx, types.ToolCall{
		ID:    call.ID,
		Name:  call.Name,
		Input: call.Input,
	})
	return subagent.ToolCallOutput{
		ToolCallID: res.ToolCallID,
		Output:     res.Output,
		Error:      res.Error,
		DurationMs: res.DurationMs,
	}, err
}

func (a *dispatcherAdapter) ListTools() []subagent.ToolDescriptor {
	var out []subagent.ToolDescriptor
	for _, name := range a.d.List() {
		t, ok := a.d.GetTool(name)
		if !ok {
			continue
		}
		desc := subagent.ToolDescriptor{
			Name:        t.Name(),
			Description: t.Description(),
		}
		if sp, ok := t.(types.SchemaProvider); ok {
			desc.ParameterSchema = sp.ParameterSchema()
		}
		out = append(out, desc)
	}
	return out
}

func (a *dispatcherAdapter) Stop() { a.d.Stop() }

// NewDispatcherFactory returns a DispatcherFactory that creates a fresh
// tools.Dispatcher for each subagent workspace. The factory registers the
// Agent tool (with isChild=true) on every dispatcher so a subagent can
// spawn its own child, but forced-foreground to block runaway fan-out.
//
// backupDir and sessionsDir are shared across all dispatchers; permCfg is
// reused verbatim so permission rules apply uniformly.
func NewDispatcherFactory(backupDir, sessionsDir string, permCfg *config.PermissionsConfig, manager *subagent.Manager) subagent.DispatcherFactory {
	return func(workDir string) (subagent.ToolDispatcher, error) {
		d, err := DefaultDispatcher(workDir, backupDir, sessionsDir, permCfg)
		if err != nil {
			return nil, err
		}
		if manager != nil {
			if err := d.Register(NewAgent(manager, true)); err != nil {
				d.Stop()
				return nil, err
			}
		}
		return &dispatcherAdapter{d: d}, nil
	}
}
