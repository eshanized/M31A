package types

import (
	"context"
	"errors"
)

type EventStore interface {
	Append(ctx context.Context, events ...Event) error
	Query(ctx context.Context, q Query) ([]Event, error)
	Subscribe(ctx context.Context, afterSeq int64) (<-chan Event, error)
	Backup(ctx context.Context, dstPath string) error
	Close() error
}

var (
	ErrEventStoreUnavailable = errors.New("event store unavailable")
	ErrEventNotFound         = errors.New("event not found")
	ErrInvalidEventPayload   = errors.New("invalid event payload")
	ErrMigrationFailed       = errors.New("migration failed")
	ErrWriteConflict         = errors.New("write conflict: another process is writing")
	ErrBackupFailed          = errors.New("backup failed")
	ErrProjectionNotFound    = errors.New("projection not found")
	ErrProjectionApplyFailed = errors.New("projection apply failed")
	ErrArtifactParseFailed   = errors.New("artifact parse failed")
	ErrKeyNotFound           = errors.New("key not found")
	ErrKeychainUnavailable   = errors.New("keychain unavailable")
)
