package tui

import (
	"context"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

func TestHealthCheckTicker_ReturnsCmd(t *testing.T) {
	cmd := HealthCheckTicker(context.Background(), time.Second)
	if cmd == nil {
		t.Error("HealthCheckTicker should return non-nil command")
	}
}

func TestNextHealthTick_ReturnsCmd(t *testing.T) {
	cmd := NextHealthTick(time.Second)
	if cmd == nil {
		t.Error("NextHealthTick should return non-nil command")
	}
}

func TestNextHealthTick_Type(t *testing.T) {
	cmd := NextHealthTick(time.Millisecond)
	if cmd == nil {
		t.Fatal("NextHealthTick returned nil")
	}

	msg := cmd()
	if _, ok := msg.(HealthCheckTickMsg); !ok {
		t.Errorf("Expected HealthCheckTickMsg, got %T", msg)
	}
}

func TestCalculateNextInterval_Default(t *testing.T) {
	result := calculateNextInterval(types.HealthStatus{Status: "live"})
	if result != types.HealthCheckInterval {
		t.Errorf("Expected %v, got %v", types.HealthCheckInterval, result)
	}
}

func TestCalculateNextInterval_RateLimit(t *testing.T) {
	result := calculateNextInterval(types.HealthStatus{Status: "slow", Error: "rate limit exceeded"})
	if result != 120*time.Second {
		t.Errorf("Expected 120s, got %v", result)
	}
}

func TestCalculateNextInterval_Offline(t *testing.T) {
	result := calculateNextInterval(types.HealthStatus{Status: "offline"})
	if result != 120*time.Second {
		t.Errorf("Expected 120s for offline, got %v", result)
	}
}

func TestCalculateNextInterval_429(t *testing.T) {
	result := calculateNextInterval(types.HealthStatus{Status: "slow", Error: "HTTP 429 Too Many Requests"})
	if result != 120*time.Second {
		t.Errorf("Expected 120s for 429, got %v", result)
	}
}

func TestHealthCheckTicker_NilCtx(t *testing.T) {
	cmd := HealthCheckTicker(nil, time.Second)
	if cmd != nil {
		t.Error("Nil context should return nil command")
	}
}

func TestHealthCheckTicker_ZeroInterval(t *testing.T) {
	cmd := HealthCheckTicker(context.Background(), nil, "", 0)
	if cmd == nil {
		t.Error("Zero interval should default to HealthCheckInterval")
	}
}

func TestNextHealthTick_ZeroInterval(t *testing.T) {
	cmd := NextHealthTick(0)
	if cmd == nil {
		t.Error("Zero interval should use default")
	}
}

func TestCalculateNextInterval_EmptyError(t *testing.T) {
	result := calculateNextInterval(types.HealthStatus{Status: "live", Error: ""})
	if result != types.HealthCheckInterval {
		t.Errorf("Empty error should return default interval, got %v", result)
	}
}

func TestCalculateNextInterval_UnrelatedError(t *testing.T) {
	result := calculateNextInterval(types.HealthStatus{Status: "live", Error: "connection refused"})
	if result != types.HealthCheckInterval {
		t.Errorf("Unrelated error should return default interval, got %v", result)
	}
}
