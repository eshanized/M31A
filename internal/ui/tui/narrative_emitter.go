package tui

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/engine/narrative"
)

// narrativeEmitter wraps a channelEmitter and intercepts workflow messages
// to feed them through the narrative engine. Original messages pass through
// unchanged; narrative results are emitted as additional messages.
type narrativeEmitter struct {
	inner  *channelEmitter
	bridge *narrative.Bridge
	engine *narrative.Engine
}

// newNarrativeEmitter creates a narrative-aware emitter wrapping the given channel.
// If cfg is non-nil, template and classification overrides from config are applied.
func newNarrativeEmitter(ch chan tea.Msg, drops *DropCounter, cfg *config.Config) *narrativeEmitter {
	var engineConfig narrative.EngineConfig
	if cfg != nil {
		engineConfig = narrative.EngineConfigWithOverrides(
			cfg.Narrative.TemplateOverrides,
			cfg.Narrative.ClassificationOverrides,
		)
	} else {
		engineConfig = narrative.DefaultEngineConfig()
	}
	return &narrativeEmitter{
		inner:  &channelEmitter{ch: ch, drops: drops},
		bridge: narrative.NewBridge(),
		engine: narrative.NewEngine(engineConfig),
	}
}

// Emit intercepts the message, processes it through the narrative engine,
// and emits both the original message and any resulting narratives.
func (ne *narrativeEmitter) Emit(msg any) {
	// Always pass through the original message.
	ne.inner.Emit(msg)

	// Try to convert to a narrative event.
	event, ok := ne.bridge.MsgToRawEvent(msg)
	if !ok {
		return
	}

	// Process through the narrative engine.
	narratives := ne.engine.ProcessEvent(event)

	// Emit each resulting narrative as a bubble message.
	for _, n := range narratives {
		ne.inner.Emit(NarrativeBubbleMsg{Narrative: n})
	}

	// Check for pending flushes from grouped events.
	pending := ne.engine.PendingNarratives()
	for _, n := range pending {
		ne.inner.Emit(NarrativeBubbleMsg{Narrative: n})
	}
}

// NarrativeEngine returns the underlying narrative engine for direct access.
func (ne *narrativeEmitter) NarrativeEngine() *narrative.Engine {
	return ne.engine
}

// NarrativeState returns the narrative state for TUI rendering.
// This is a convenience method; the state should be managed by AppState.
func (ne *narrativeEmitter) NarrativeBridge() *narrative.Bridge {
	return ne.bridge
}

// EmitNarrative manually emits a narrative message (e.g., for phase transitions
// detected at the TUI level).
func (ne *narrativeEmitter) EmitNarrative(n narrative.NarrativeObject) {
	ne.inner.Emit(NarrativeBubbleMsg{Narrative: n})
}

// LogDropped returns the current drop count for observability.
func (ne *narrativeEmitter) LogDropped() int64 {
	return ne.inner.drops.Load()
}
