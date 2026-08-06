package extensions

import (
	"context"
	"encoding/json"
	"fmt"
	"time"
)

// PhaseHookAdapter wraps a SubprocessManager to implement PhaseHookHandler.
type PhaseHookAdapter struct {
	name        string
	procManager *SubprocessManager
	emitter     MsgEmitter
}

var (
	_ PhaseHookHandler = (*PhaseHookAdapter)(nil)
)

// NewPhaseHookAdapter creates a new PhaseHookAdapter.
func NewPhaseHookAdapter(name string, proc *SubprocessManager, emitter MsgEmitter) *PhaseHookAdapter {
	return &PhaseHookAdapter{
		name:        name,
		procManager: proc,
		emitter:     emitter,
	}
}

// PrePhase is called before a workflow phase begins.
func (a *PhaseHookAdapter) PrePhase(ctx context.Context, payload PhaseHookPayload) error {
	req := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      a.procManager.nextRequestID(),
		Method:  MethodHookPrePhase,
	}

	params := HookPrePhaseParams{Payload: payload}
	paramsData, err := json.Marshal(params)
	if err != nil {
		return fmt.Errorf("marshal params: %w", err)
	}
	req.Params = paramsData

	timeout, err := payload.getTimeout()
	if err != nil {
		timeout = 30 * time.Second
	}

	resp, err := a.procManager.Call(ctx, req, timeout)
	if err != nil {
		return err
	}

	if resp.Error != nil {
		return fmt.Errorf("pre_phase hook error: %s", resp.Error.Message)
	}

	return nil
}

// PostPhase is called after a workflow phase completes.
func (a *PhaseHookAdapter) PostPhase(ctx context.Context, payload PhaseHookPayload, result *PhaseResult) error {
	req := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      a.procManager.nextRequestID(),
		Method:  MethodHookPostPhase,
	}

	params := HookPostPhaseParams{
		Payload: payload,
		Result:  *result,
	}
	paramsData, err := json.Marshal(params)
	if err != nil {
		return fmt.Errorf("marshal params: %w", err)
	}
	req.Params = paramsData

	timeout, err := payload.getTimeout()
	if err != nil {
		timeout = 30 * time.Second
	}

	resp, err := a.procManager.Call(ctx, req, timeout)
	if err != nil {
		return err
	}

	if resp.Error != nil {
		return fmt.Errorf("post_phase hook error: %s", resp.Error.Message)
	}

	return nil
}

// getTimeout extracts timeout from hook config if available.
// This is a helper method on PhaseHookPayload.
func (p PhaseHookPayload) getTimeout() (time.Duration, error) {
	if len(p.ExtensionConfig) == 0 {
		return 0, fmt.Errorf("no extension config")
	}

	var cfg struct {
		Timeout string `json:"timeout"`
	}
	if err := json.Unmarshal(p.ExtensionConfig, &cfg); err != nil {
		return 0, err
	}

	if cfg.Timeout == "" {
		return 0, fmt.Errorf("no timeout in config")
	}

	return time.ParseDuration(cfg.Timeout)
}