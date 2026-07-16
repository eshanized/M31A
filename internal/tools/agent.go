package tools

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tools/subagent"
	"github.com/eshanized/M31A/pkg/types"
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
	// depth tracks nesting depth (0 = root, 1 = child, 2 = grandchild).
	depth int
	// profiles holds user-configurable agent profile overrides for
	// dynamic description generation.
	profiles map[string]config.SubagentProfileConfig
}

// MaxAgentDepth is the maximum nesting depth for subagents.
// Depth 0 is the root agent, depth 1 is a child, depth 2 is a grandchild.
// Going deeper is blocked to prevent runaway resource consumption.
const MaxAgentDepth = 2

// NewAgent creates a new Agent tool instance.
func NewAgent(m *subagent.Manager, isChild bool, depth int, profiles map[string]config.SubagentProfileConfig) *Agent {
	return &Agent{manager: m, isChild: isChild, depth: depth, profiles: profiles}
}

func (t *Agent) Name() string               { return "Agent" }
func (t *Agent) RiskLevel() types.RiskLevel { return types.RiskSafe }

func (t *Agent) Description() string {
	var sb strings.Builder
	sb.WriteString(`Launch a new agent to handle complex, multistep tasks autonomously.

When using the Agent tool, you must specify a subagent_type parameter to select which agent type to use.

When NOT to use the Agent tool:
- If you want to read a specific file path, use FileRead or Glob instead
- If you are searching for a specific pattern, use Grep instead
- If no available agent is a good fit for the task, use other tools directly

Usage notes:
1. Launch multiple agents concurrently whenever possible to maximize performance
2. Once you have delegated work, do not duplicate that work yourself
3. When the agent is done, it returns a single message with its findings
4. Each agent invocation starts fresh unless you provide task_id to resume
5. Clearly tell the agent whether you expect it to write code or just research

Available agent types:`)
	sb.WriteString("\n")

	for _, p := range subagent.ListSubagentProfiles(t.profiles) {
		fmt.Fprintf(&sb, "- %s: %s\n", p.Name, p.Description)
	}

	sb.WriteString(`
Parameters:
- description (required, 3-5 words): short task label shown in the TUI.
- prompt (required): full task description with concrete objectives.
- subagent_type (required): the type of specialized agent to use.
- name (optional): stable label for the subagent.
- isolation: "worktree" (own git worktree, default) or "default" (shared dir).
- run_in_background: true (default, async) or false (block until done).

Max 8 run concurrently. Do not spawn more than needed — each subagent costs tokens.`)
	return sb.String()
}

// ParameterSchema returns the JSON schema exposed to the LLM.
func (t *Agent) ParameterSchema() string {
	return `{
  "type": "object",
  "properties": {
    "description":     {"type": "string", "description": "Short 3-5 word task label"},
    "prompt":          {"type": "string", "description": "Full task description"},
    "subagent_type":   {"type": "string", "description": "The type of specialized agent to use"},
    "name":            {"type": "string", "description": "Optional stable label"},
    "isolation":       {"type": "string", "enum": ["worktree","default"], "default": "worktree"},
    "run_in_background": {"type": "boolean", "default": true}
  },
  "required": ["description", "prompt", "subagent_type"]
}`
}

// Execute validates input, spawns the subagent, and returns its ID + status.
func (t *Agent) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	if t.manager == nil {
		return types.ToolResult{Error: "Agent tool: subagent manager not configured"}, nil
	}

	// Enforce depth limit (L2)
	if t.depth >= MaxAgentDepth {
		return types.ToolResult{Error: fmt.Sprintf("Agent: maximum subagent depth (%d) reached. Use foreground mode or handle the task directly.", MaxAgentDepth)}, nil
	}

	var req struct {
		Description     string `json:"description"`
		Prompt          string `json:"prompt"`
		SubagentType    string `json:"subagent_type"`
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
	if req.SubagentType == "" {
		req.SubagentType = "general"
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
		Description:  req.Description,
		Prompt:       req.Prompt,
		SubagentType: req.SubagentType,
		Name:         req.Name,
		Isolation:    iso,
		Background:   background,
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

func (a *dispatcherAdapter) UnregisterTool(name string) { a.d.Unregister(name) }

// NewDispatcherFactory returns a DispatcherFactory that creates a fresh
// tools.Dispatcher for each subagent workspace. The factory registers the
// Agent tool (with isChild=true) on every dispatcher so a subagent can
// spawn its own child, but forced-foreground to block runaway fan-out.
//
// backupDir and sessionsDir are shared across all dispatchers; permCfg is
// reused verbatim so permission rules apply uniformly.
//
// NewDispatcherFactory creates a new DispatcherFactory for subagents.
func NewDispatcherFactory(backupDir, sessionsDir string, permCfg *config.PermissionsConfig, toolsCfg *config.ToolsConfig, manager *subagent.Manager, profiles map[string]config.SubagentProfileConfig) subagent.DispatcherFactory {
	return func(workDir string) (subagent.ToolDispatcher, error) {
		d, err := DefaultDispatcher(workDir, backupDir, sessionsDir, permCfg, toolsCfg)
		if err != nil {
			return nil, err
		}
		if manager != nil {
			if err := d.Register(NewAgent(manager, true, 1, profiles)); err != nil {
				d.Stop()
				return nil, err
			}
		}
		return &dispatcherAdapter{d: d}, nil
	}
}
