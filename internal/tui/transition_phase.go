package tui

// screenOrder maps screens to a navigation depth for forward/back detection.
// The workflow sequence is: Home -> GoalInput -> Discuss -> Plan -> Execute -> Verify -> RuntimeCheck -> Ship
var screenOrder = map[Screen]int{
	ScreenHome:             -1,
	ScreenREPL:             0,
	ScreenDashboard:        1,
	ScreenGoalInput:        2,
	ScreenDiscuss:          3,
	ScreenPlan:             4,
	ScreenExecute:          5,
	ScreenVerify:           6,
	ScreenRuntimeCheck:     7,
	ScreenShip:             8,
	ScreenSettings:         10,
	ScreenModelSelector:    10,
	ScreenResume:           11,
	ScreenSessionDetail:    12,
	ScreenHelp:             13,
	ScreenLedger:           14,
	ScreenRollback:         15,
	ScreenMetrics:          16,
	ScreenConfig:           17,
	ScreenBisect:           18,
	ScreenNotifications:    20,
	ScreenChatHistory:      21,
	ScreenFileExplorer:     22,
	ScreenToolDetail:       23,
	ScreenPhaseModelPicker: 24,
	ScreenGhostPicker:      25,
	ScreenGhostOutput:      26,
}

// isBackNavigation returns true if navigating from `from` to `to` is
// a backward navigation (escape/back action).
func isBackNavigation(from, to Screen) bool {
	if to == ScreenREPL && from != ScreenREPL {
		return true
	}
	fromOrder, fromOK := screenOrder[from]
	toOrder, toOK := screenOrder[to]
	if fromOK && toOK {
		return toOrder < fromOrder
	}
	return false
}
