package types

import (
	"testing"
	"time"
)

func TestConstants(t *testing.T) {
	t.Parallel()
	if ModelCacheTTL != 5*time.Minute {
		t.Errorf("expected 5min, got %v", ModelCacheTTL)
	}
	if MaxToolOutputChars != 10_000 {
		t.Errorf("expected 10000, got %d", MaxToolOutputChars)
	}
	if MaxHealAttempts != 2 {
		t.Errorf("expected 2, got %d", MaxHealAttempts)
	}
	if MaxPlanRetries != 3 {
		t.Errorf("expected 3, got %d", MaxPlanRetries)
	}
	if SessionIDLength != 8 {
		t.Errorf("expected 8, got %d", SessionIDLength)
	}
	if BashTimeout != 30*time.Minute {
		t.Errorf("expected 30min, got %v", BashTimeout)
	}
	if BashOutputLimit != 50_000 {
		t.Errorf("expected 50000, got %d", BashOutputLimit)
	}
	if DefaultContextLength != 128_000 {
		t.Errorf("expected 128000, got %d", DefaultContextLength)
	}
	if MaxSessionFileSize != 50*1024*1024 {
		t.Errorf("expected 50MB, got %d", MaxSessionFileSize)
	}
	if DefaultPermissionTimeout != 300 {
		t.Errorf("expected 300, got %d", DefaultPermissionTimeout)
	}
}

func TestHealthStatusConstants(t *testing.T) {
	t.Parallel()
	if HealthStatusLive != "live" {
		t.Errorf("expected 'live', got %q", HealthStatusLive)
	}
	if HealthStatusSlow != "slow" {
		t.Errorf("expected 'slow', got %q", HealthStatusSlow)
	}
	if HealthStatusOffline != "offline" {
		t.Errorf("expected 'offline', got %q", HealthStatusOffline)
	}
}

func TestURLConstants(t *testing.T) {
	t.Parallel()
	if DefaultOpenRouterBaseURL == "" {
		t.Error("expected non-empty OpenRouter URL")
	}
	if DefaultZenBaseURL == "" {
		t.Error("expected non-empty Zen URL")
	}
}
