package types

import "time"

// CommitInfo represents metadata about a git commit.
type CommitInfo struct {
	Hash      string    `json:"hash"`
	ShortHash string    `json:"short_hash"`
	Author    string    `json:"author"`
	Message   string    `json:"message"`
	Timestamp time.Time `json:"timestamp"`
}
