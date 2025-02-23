package tui

import "time"

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
