package tools

import (
	"context"
	"testing"

	"golang.org/x/time/rate"
)

func BenchmarkRateLimiterWait(b *testing.B) {
	limiter := rate.NewLimiter(rate.Limit(ToolRateLimitPerSec), ToolRateLimitBurst)
	ctx := context.Background()
	b.ResetTimer()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		_ = limiter.Wait(ctx)
	}
}

func BenchmarkRateLimiterBurst(b *testing.B) {
	limiter := rate.NewLimiter(rate.Limit(ToolRateLimitPerSec), ToolRateLimitBurst)
	b.ResetTimer()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		_ = limiter.Allow()
	}
}

func BenchmarkRateLimiterContended(b *testing.B) {
	limiter := rate.NewLimiter(rate.Limit(ToolRateLimitPerSec), ToolRateLimitBurst)
	ctx := context.Background()
	b.ResetTimer()
	b.ReportAllocs()
	b.RunParallel(func(pb *testing.PB) {
		for pb.Next() {
			_ = limiter.Wait(ctx)
		}
	})
}
