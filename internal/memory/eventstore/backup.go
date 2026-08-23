package eventstore

import (
	"context"
	"os"
	"path/filepath"

	coreerrors "github.com/eshanized/M31A/internal/core/errors"
	_ "modernc.org/sqlite"
)

func (s *SQLiteEventStore) Backup(ctx context.Context, dstPath string) error {
	// Ensure destination directory exists
	if err := os.MkdirAll(filepath.Dir(dstPath), 0755); err != nil {
		return coreerrors.Wrap(err, "create backup directory")
	}

	// Use SQLite VACUUM INTO for consistent backup
	// Requires SQLite 3.27+ (modernc.org/sqlite has it)
	// Path must be a string literal in SQL
	query := "VACUUM INTO '" + dstPath + "'"
	_, err := s.db.ExecContext(ctx, query)
	if err != nil {
		return coreerrors.Wrap(err, "vacuum into backup")
	}

	// Verify the backup was created
	if _, err := os.Stat(dstPath); err != nil {
		return coreerrors.Wrap(err, "verify backup file")
	}

	return nil
}