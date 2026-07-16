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

// GitClient abstracts git operations for testability.
// Consumers (rollback, bisect, workflow) should depend on this interface
// rather than the concrete *git.Git implementation.
type GitClient interface {
	Init() error
	IsRepo() bool
	Add(paths ...string) error
	AddAll() error
	Commit(message string) error
	Log(lastN int) ([]CommitInfo, error)
	LogSince(since time.Time) ([]CommitInfo, error)
	Diff(target ...string) (string, error)
	DiffStat(target ...string) (string, error)
	CurrentBranch() (string, error)
	RevParse(ref string) (string, error)
	IsDirty() (bool, error)
	HasUncommittedChanges() (bool, error)
	Run(args ...string) (string, error)
}
