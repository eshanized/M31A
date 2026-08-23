package eventstore

import (
	"context"
	"database/sql"
	"encoding/json"
	"time"

	coreerrors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/google/uuid"
	_ "modernc.org/sqlite"
)

func (s *SQLiteEventStore) Query(ctx context.Context, q types.Query) ([]types.Event, error) {
	query := "SELECT seq, id, type, timestamp, run_id, session_id, payload, metadata FROM events WHERE 1=1"
	args := []any{}

	if q.RunID != nil {
		query += " AND run_id = ?"
		args = append(args, q.RunID.String())
	}
	if q.SessionID != nil {
		query += " AND session_id = ?"
		args = append(args, q.SessionID.String())
	}
	if q.Type != nil {
		query += " AND type = ?"
		args = append(args, string(*q.Type))
	}
	if q.AfterSeq > 0 {
		query += " AND seq > ?"
		args = append(args, q.AfterSeq)
	}
	if q.BeforeSeq > 0 {
		query += " AND seq < ?"
		args = append(args, q.BeforeSeq)
	}
	query += " ORDER BY seq ASC"
	if q.Limit > 0 {
		query += " LIMIT ?"
		args = append(args, q.Limit)
	}
	if q.Offset > 0 {
		query += " OFFSET ?"
		args = append(args, q.Offset)
	}

	rows, err := s.db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, coreerrors.Wrap(err, "query events")
	}
	defer rows.Close()

	events := make([]types.Event, 0)
	for rows.Next() {
		var evt types.Event
		var runID, sessionID sql.NullString
		var payload, metadata []byte
		var timestamp int64
		if err := rows.Scan(&evt.Seq, &evt.ID, &evt.Type, &timestamp, &runID, &sessionID, &payload, &metadata); err != nil {
			return nil, coreerrors.Wrap(err, "scan event")
		}
		evt.Timestamp = time.Unix(0, timestamp).UTC()
		if runID.Valid {
			parsed, _ := uuid.Parse(runID.String)
			evt.RunID = &parsed
		}
		if sessionID.Valid {
			parsed, _ := uuid.Parse(sessionID.String)
			evt.SessionID = &parsed
		}
		evt.Payload = payload
		var meta types.EventMetadata
		if len(metadata) > 0 {
			_ = json.Unmarshal(metadata, &meta)
		}
		evt.Metadata = meta
		events = append(events, evt)
	}
	return events, rows.Err()
}

func (s *SQLiteEventStore) QueryByRun(ctx context.Context, runID uuid.UUID, afterSeq int64, limit int) ([]types.Event, error) {
	return s.Query(ctx, types.Query{
		RunID:    &runID,
		AfterSeq: afterSeq,
		Limit:    limit,
	})
}