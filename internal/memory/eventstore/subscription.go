package eventstore

import (
	"context"
	"log/slog"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	_ "modernc.org/sqlite"
)

const SubscriptionPollInterval = 100 * time.Millisecond

func (s *SQLiteEventStore) Subscribe(ctx context.Context, afterSeq int64) (<-chan types.Event, error) {
	ch := make(chan types.Event, 100)
	go func() {
		defer close(ch)
		ticker := time.NewTicker(SubscriptionPollInterval)
		defer ticker.Stop()
		var lastSeq = afterSeq
		for {
			select {
			case <-ctx.Done():
				return
			case <-ticker.C:
				events, err := s.Query(ctx, types.Query{AfterSeq: lastSeq, Limit: 100})
				if err != nil {
					slog.Error("subscription query failed", "error", err)
					continue
				}
				for _, evt := range events {
					select {
					case ch <- evt:
						lastSeq = evt.Seq
					case <-ctx.Done():
						return
					}
				}
			}
		}
	}()
	return ch, nil
}