package eventstore

import (
	"context"
	"encoding/json"
	"time"

	coreerrors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/google/uuid"
	_ "modernc.org/sqlite"
)

func (s *SQLiteEventStore) AppendEvents(ctx context.Context, events []types.Event) error {
	if len(events) == 0 {
		return nil
	}
	tx, err := s.db.BeginTx(ctx, nil)
if err != nil {
			return coreerrors.Wrap(err, "begin transaction")
		}
	defer tx.Rollback()

	stmt, err := tx.PrepareContext(ctx, `
		INSERT INTO events (id, type, timestamp, run_id, session_id, payload, metadata)
		VALUES (?, ?, ?, ?, ?, ?, ?)
	`)
	if err != nil {
		return coreerrors.Wrap(err, "prepare statement")
	}
	defer stmt.Close()

	for i := range events {
		if events[i].ID == (uuid.UUID{}) {
			events[i].ID = uuid.New()
		}
		if events[i].Timestamp.IsZero() {
			events[i].Timestamp = time.Now().UTC()
		}

		payload, err := json.Marshal(events[i].Payload)
		if err != nil {
			return coreerrors.Wrapf(err, "marshal payload for event %d", i)
		}
		metadata, err := json.Marshal(events[i].Metadata)
		if err != nil {
			return coreerrors.Wrapf(err, "marshal metadata for event %d", i)
		}

		var runID, sessionID any
		if events[i].RunID != nil {
			runID = events[i].RunID.String()
		}
		if events[i].SessionID != nil {
			sessionID = events[i].SessionID.String()
		}

		if _, err := stmt.ExecContext(ctx,
			events[i].ID.String(),
			string(events[i].Type),
			events[i].Timestamp.UnixNano(),
			runID,
			sessionID,
			payload,
			nullString(string(metadata)),
		); err != nil {
			return coreerrors.Wrapf(err, "exec event %d", i)
		}
	}

	if err := tx.Commit(); err != nil {
		return coreerrors.Wrap(err, "commit transaction")
	}
	return nil
}

func (s *SQLiteEventStore) Append(ctx context.Context, events ...types.Event) error {
	return s.AppendEvents(ctx, events)
}