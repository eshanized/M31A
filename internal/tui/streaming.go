// Package tui implements the Bubble Tea TUI for M31A.
//
// This file re-exports streaming types from the streaming sub-package so that
// existing code within the tui package can reference them without explicit
// imports. New code should import the streaming package directly.
package tui

import "github.com/eshanized/M31A/internal/tui/streaming"

// Streaming type re-exports
type (
	StreamMsg             = streaming.StreamMsg
	StreamDoneMsg         = streaming.StreamDoneMsg
	StreamErrorMsg        = streaming.StreamErrorMsg
	TickMsg               = streaming.TickMsg
	AgentStreamMsg        = streaming.AgentStreamMsg
	AgentToolStartMsg     = streaming.AgentToolStartMsg
	AgentToolDoneMsg      = streaming.AgentToolDoneMsg
	AgentToolProgressMsg  = streaming.AgentToolProgressMsg
	AgentDoneMsg          = streaming.AgentDoneMsg
	AgentErrorMsg         = streaming.AgentErrorMsg
	AgentThinkingMsg      = streaming.AgentThinkingMsg
	AgentIterationDoneMsg = streaming.AgentIterationDoneMsg
	AgentIterationMsg     = streaming.AgentIterationMsg
)

// Streaming function re-exports
var (
	StartStreamCmd             = streaming.StartStreamCmd
	StreamTickCmd              = streaming.StreamTickCmd
	AgentLoop                  = streaming.AgentLoop
	BuildAgentMessages         = streaming.BuildAgentMessages
	LoadProjectContextForAgent = streaming.LoadProjectContextForAgent
)
