package tui

import (
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/types"
)

// handlePermissionRequest handles PermissionRequestMsg by showing the permission modal.
func (m *AppState) handlePermissionRequest(msg PermissionRequestMsg) (tea.Model, tea.Cmd) {
	if m.permissionModalActive {
		m.pendingPermissionRequests = append(m.pendingPermissionRequests, msg)
		return m, permissionListenerCmd(m.shutdownCtx, m.dispatcher)
	}
	m.permissionModalActive = true
	m.pendingPermissionRequestID = msg.Request.ID // RC-2: store request ID for response correlation
	m.prevScreen = m.screen
	m.screen = ScreenPermission
	t := m.themeManager.Current()
	timeout := time.Duration(msg.Request.TimeoutSecs) * time.Second
	if timeout <= 0 {
		timeout = time.Duration(types.DefaultPermissionTimeout) * time.Second
	}
	pm := components.NewPermissionModal(msg.Request, t, timeout)
	m.permissionModal = pm
	listenerCmds := m.listenerCmds()
	return m, tea.Batch(
		append(listenerCmds, tea.Every(100*time.Millisecond, func(t time.Time) tea.Msg {
			return PermissionTickMsg{}
		}))...,
	)
}

// handlePermissionResponse handles PermissionResponseMsg by approving/denying the permission.
func (m *AppState) handlePermissionResponse(msg PermissionResponseMsg) (tea.Model, tea.Cmd) {
	m.dispatcher.ApprovePermission(m.pendingPermissionRequestID, msg.Response.Allowed, msg.Response.Remember)
	m.screen = m.prevScreen
	m.permissionModal = nil
	m.permissionModalActive = false
	m.pendingPermissionRequestID = 0
	if len(m.pendingPermissionRequests) > 0 {
		next := m.pendingPermissionRequests[0]
		m.pendingPermissionRequests = m.pendingPermissionRequests[1:]
		return m.handlePermissionRequest(next)
	}
	return m, tea.Batch(m.listenerCmds()...)
}

// handlePermissionTick handles PermissionTickMsg for the permission modal timeout.
func (m *AppState) handlePermissionTick() (tea.Model, tea.Cmd) {
	if m.screen == ScreenPermission && m.permissionModal != nil {
		m.permissionModal.Tick()
		if m.permissionModal.Remaining() <= 0 {
			resp := m.permissionModal.Deny()
			m.dispatcher.ApprovePermission(m.pendingPermissionRequestID, resp.Allowed, resp.Remember)
			m.screen = m.prevScreen
			m.permissionModal = nil
			m.permissionModalActive = false
			m.pendingPermissionRequestID = 0
			if len(m.pendingPermissionRequests) > 0 {
				next := m.pendingPermissionRequests[0]
				m.pendingPermissionRequests = m.pendingPermissionRequests[1:]
				return m.handlePermissionRequest(next)
			}
			return m, tea.Batch(m.listenerCmds()...)
		}
		return m, tea.Every(100*time.Millisecond, func(t time.Time) tea.Msg {
			return PermissionTickMsg{}
		})
	}
	return m, nil
}