package tui

import (
	"time"

	tea "github.com/charmbracelet/bubbletea"
)

type KeyContext string

const (
	CtxGlobal    KeyContext = "global"
	CtxREPL      KeyContext = "repl"
	CtxPalette   KeyContext = "palette"
	CtxSidebar   KeyContext = "sidebar"
	CtxSettings  KeyContext = "settings"
	CtxModelSel  KeyContext = "modelselector"
	CtxResume    KeyContext = "resume"
	CtxPermModal KeyContext = "permission"
	CtxFirstRun  KeyContext = "firstrun"
)

type KeyAction func() tea.Cmd

type KeyBinding struct {
	Key         string
	Description string
	Action      KeyAction
	Context     KeyContext
}

type LeaderTimeoutMsg struct{}

type KeyActionMsg struct {
	Action string
}

type KeyRegistry struct {
	bindings map[KeyContext][]KeyBinding

	leaderActive  bool
	leaderKey     string
	leaderTimeout time.Duration
	leaderTimer   *time.Timer

	whichKeyActive bool
}

type KeyRegistryOpts struct {
	LeaderKey     string
	LeaderTimeout time.Duration
}

func NewKeyRegistry(opts KeyRegistryOpts) *KeyRegistry {
	if opts.LeaderKey == "" {
		opts.LeaderKey = "ctrl+x"
	}
	if opts.LeaderTimeout == 0 {
		opts.LeaderTimeout = 1 * time.Second
	}
	return &KeyRegistry{
		bindings:      make(map[KeyContext][]KeyBinding),
		leaderKey:     opts.LeaderKey,
		leaderTimeout: opts.LeaderTimeout,
	}
}

func (r *KeyRegistry) Register(ctx KeyContext, key, description string, action KeyAction) {
	r.bindings[ctx] = append(r.bindings[ctx], KeyBinding{
		Key:         key,
		Description: description,
		Action:      action,
		Context:     ctx,
	})
}

func (r *KeyRegistry) Handle(key string, ctx KeyContext) (bool, tea.Cmd) {
	if r.leaderActive {
		r.cancelLeaderTimer()
		r.leaderActive = false

		chordKey := r.leaderKey + " " + key
		for _, b := range r.bindings[ctx] {
			if b.Key == chordKey {
				if b.Action == nil {
					return true, nil
				}
				return true, b.Action()
			}
		}
		for _, b := range r.bindings[CtxGlobal] {
			if b.Key == chordKey {
				if b.Action == nil {
					return true, nil
				}
				return true, b.Action()
			}
		}
	}

	if key == r.leaderKey {
		r.leaderActive = true
		return true, tea.Tick(r.leaderTimeout, func(time.Time) tea.Msg {
			return LeaderTimeoutMsg{}
		})
	}

	for _, b := range r.bindings[ctx] {
		if b.Key == key {
			if b.Action == nil {
				return true, nil
			}
			return true, b.Action()
		}
	}
	for _, b := range r.bindings[CtxGlobal] {
		if b.Key == key {
			if b.Action == nil {
				return true, nil
			}
			return true, b.Action()
		}
	}

	return false, nil
}

func (r *KeyRegistry) IsLeaderActive() bool {
	return r.leaderActive
}

func (r *KeyRegistry) DeactivateLeader() {
	r.leaderActive = false
	r.cancelLeaderTimer()
}

func (r *KeyRegistry) cancelLeaderTimer() {
	if r.leaderTimer != nil {
		r.leaderTimer.Stop()
		r.leaderTimer = nil
	}
}

func (r *KeyRegistry) GetContextBindings(ctx KeyContext) []KeyBinding {
	var result []KeyBinding
	result = append(result, r.bindings[CtxGlobal]...)
	result = append(result, r.bindings[ctx]...)
	return result
}
