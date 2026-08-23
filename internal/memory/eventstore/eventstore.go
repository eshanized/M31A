package eventstore

import (
	"context"
	"database/sql"
	"errors"

	coreerrors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
	_ "modernc.org/sqlite"
)

const schemaSQL = `
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

CREATE TABLE IF NOT EXISTS events (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,
    id          TEXT NOT NULL UNIQUE,
    type        TEXT NOT NULL,
    timestamp   INTEGER NOT NULL,
    run_id      TEXT,
    session_id  TEXT,
    payload     BLOB NOT NULL,
    metadata    BLOB
);

CREATE INDEX IF NOT EXISTS idx_events_run_seq      ON events(run_id, seq);
CREATE INDEX IF NOT EXISTS idx_events_session_seq  ON events(session_id, seq);
CREATE INDEX IF NOT EXISTS idx_events_type_seq     ON events(type, seq);
CREATE INDEX IF NOT EXISTS idx_events_timestamp    ON events(timestamp);

CREATE TABLE IF NOT EXISTS projections (
    name        TEXT PRIMARY KEY,
    last_seq    INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS migrations (
    version     INTEGER PRIMARY KEY,
    description TEXT NOT NULL,
    applied_at  INTEGER NOT NULL
);
`

var (
	ErrEventStoreUnavailable = errors.New("event store unavailable")
	ErrWriteConflict         = errors.New("write conflict: another process is writing")
)

type SQLiteEventStore struct {
	db *sql.DB
}

func NewEventStore(dbPath string) (*SQLiteEventStore, error) {
	dsn := "file:" + dbPath + "?_pragma=journal_mode(WAL)&_pragma=foreign_keys(1)&_pragma=busy_timeout(5000)&_time_format=sqlite"
	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, coreerrors.Wrap(err, "open sqlite database")
	}
	if _, err := db.ExecContext(context.Background(), schemaSQL); err != nil {
		db.Close()
		return nil, coreerrors.Wrap(err, "execute schema")
	}
	return &SQLiteEventStore{db: db}, nil
}

func (s *SQLiteEventStore) Close() error {
	return s.db.Close()
}

func (s *SQLiteEventStore) DB() *sql.DB {
	return s.db
}

func (s *SQLiteEventStore) Subscribe(ctx context.Context, afterSeq int64) (<-chan types.Event, error) {
	ch := make(chan types.Event, 100)
	return ch, nil
}

func (s *SQLiteEventStore) Backup(ctx context.Context, dstPath string) error {
	return coreerrors.ErrNotImplemented
}

func nullString(s string) any {
	if s == "" {
		return nil
	}
	return s
}