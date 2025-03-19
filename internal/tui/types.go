package tui

import (
	"time"

	"github.com/eshanized/M31A/internal/tools"
)

type Screen int

const (
	ScreenFirstRun Screen = iota
	ScreenREPL
	ScreenModelSelector
	ScreenSettings
	ScreenResume
	ScreenPermission
)

type AppMsg struct {
	Screen    Screen
	SessionID string   // populated by resume screen on selection
	Health    *HealthUpdateMsg
	Provider  *ProviderSwitchMsg
	InitError error
}

type HealthUpdateMsg struct {
	Status    string
	LatencyMs int64
	Provider  string
	Error     string
}

type HealthCheckTickMsg struct {
	Time time.Time
}

type ProviderSwitchMsg struct {
	Provider string
}

type ErrorMsg struct {
	Err error
}

type PermissionRequestMsg struct {
	Request tools.PermissionRequest
}

type PermissionResponseMsg struct {
	Response tools.PermissionResponse
}
