package session

import "time"

// SessionInfo holds display metadata for session listings.
type SessionInfo struct {
	ID           string    `json:"id"`
	Model        string    `json:"model"`
	Provider     string    `json:"provider"`
	StartedAt    time.Time `json:"started_at"`
	LastModified time.Time `json:"last_modified"`
	MessageCount int       `json:"message_count"`
	Corrupted    bool      `json:"corrupted"`
}
