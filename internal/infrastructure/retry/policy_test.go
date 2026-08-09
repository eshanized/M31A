package retry

import (
	"context"
	"errors"
	"net/http"
	"strconv"
	"testing"
	"time"
)

func TestDefaultPolicy(t *testing.T) {
	p := DefaultPolicy()
	if p.MaxAttempts != 3 {
		t.Errorf("MaxAttempts = %d, want 3", p.MaxAttempts)
	}
	if p.InitialDelay != 1*time.Second {
		t.Errorf("InitialDelay = %v, want 1s", p.InitialDelay)
	}
	if p.MaxDelay != 30*time.Second {
		t.Errorf("MaxDelay = %v, want 30s", p.MaxDelay)
	}
	if p.BackoffFactor != 2.0 {
		t.Errorf("BackoffFactor = %f, want 2.0", p.BackoffFactor)
	}
}

func TestPolicy_Delay(t *testing.T) {
	p := &Policy{
		MaxAttempts:   3,
		InitialDelay:  100 * time.Millisecond,
		MaxDelay:      5 * time.Second,
		BackoffFactor: 2.0,
	}

	tests := []struct {
		name    string
		attempt int
		want    time.Duration
	}{
		{"attempt 1", 1, 100 * time.Millisecond},
		{"attempt 2", 2, 200 * time.Millisecond},
		{"attempt 3", 3, 400 * time.Millisecond},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := p.Delay(tt.attempt, nil)
			if got != tt.want {
				t.Errorf("Delay(%d) = %v, want %v", tt.attempt, got, tt.want)
			}
		})
	}
}

func TestPolicy_Delay_MaxCap(t *testing.T) {
	p := &Policy{
		MaxAttempts:   10,
		InitialDelay:  1 * time.Second,
		MaxDelay:      5 * time.Second,
		BackoffFactor: 2.0,
	}

	got := p.Delay(10, nil)
	if got != 5*time.Second {
		t.Errorf("Delay(10) = %v, want 5s (capped)", got)
	}
}

func TestPolicy_Delay_RetryAfterMs(t *testing.T) {
	p := &Policy{InitialDelay: 1 * time.Second}
	headers := http.Header{}
	headers.Set("Retry-After-Ms", "500")

	got := p.Delay(1, headers)
	if got != 500*time.Millisecond {
		t.Errorf("Delay with Retry-After-Ms = %v, want 500ms", got)
	}
}

func TestPolicy_Delay_RetryAfterSeconds(t *testing.T) {
	p := &Policy{InitialDelay: 1 * time.Second}
	headers := http.Header{}
	headers.Set("Retry-After", "3")

	got := p.Delay(1, headers)
	if got != 3*time.Second {
		t.Errorf("Delay with Retry-After = %v, want 3s", got)
	}
}

func TestPolicy_Delay_RetryAfterRFC1123(t *testing.T) {
	p := &Policy{InitialDelay: 1 * time.Second}
	headers := http.Header{}
	futureTime := time.Now().Add(2 * time.Second).UTC().Format(time.RFC1123)
	headers.Set("Retry-After", futureTime)

	got := p.Delay(1, headers)
	if got < 1*time.Second || got > 3*time.Second {
		t.Errorf("Delay with RFC1123 Retry-After = %v, expected ~2s", got)
	}
}

func TestPolicy_Delay_RetryAfterPastDate(t *testing.T) {
	p := &Policy{InitialDelay: 1 * time.Second}
	headers := http.Header{}
	pastTime := time.Now().Add(-1 * time.Second).UTC().Format(time.RFC1123)
	headers.Set("Retry-After", pastTime)

	got := p.Delay(1, headers)
	if got != 0 {
		t.Errorf("Delay with past Retry-After = %v, want 0", got)
	}
}

func TestPolicy_Delay_InvalidRetryAfterMs(t *testing.T) {
	p := &Policy{InitialDelay: 1 * time.Second, MaxDelay: 30 * time.Second, BackoffFactor: 2.0}
	headers := http.Header{}
	headers.Set("Retry-After-Ms", "invalid")

	got := p.Delay(1, headers)
	// Invalid Retry-After-Ms should fall back to exponential backoff (>= 0)
	if got < 0 {
		t.Errorf("Delay with invalid Retry-After-Ms = %v, expected non-negative", got)
	}
}

func TestPolicy_Delay_InvalidRetryAfterSeconds(t *testing.T) {
	p := &Policy{InitialDelay: 1 * time.Second, MaxDelay: 30 * time.Second, BackoffFactor: 2.0}
	headers := http.Header{}
	headers.Set("Retry-After", "invalid")

	got := p.Delay(1, headers)
	// Invalid Retry-After should fall back to exponential backoff (>= 0)
	if got < 0 {
		t.Errorf("Delay with invalid Retry-After = %v, expected non-negative", got)
	}
}

func TestClassifyError(t *testing.T) {
	tests := []struct {
		name      string
		err       error
		wantClass ErrorClass
	}{
		{"nil", nil, ErrorClassUnknown},
		{"context overflow", errors.New("context overflow"), ErrorClassContextOverflow},
		{"context too long", errors.New("context too long"), ErrorClassContextOverflow},
		{"context exceeds", errors.New("context exceeds limit"), ErrorClassContextOverflow},
		{"rate limit", errors.New("rate limit exceeded"), ErrorClassRateLimit},
		{"too many requests", errors.New("too many requests"), ErrorClassRateLimit},
		{"429", errors.New("HTTP 429"), ErrorClassRateLimit},
		{"overloaded", errors.New("server overloaded"), ErrorClassOverloaded},
		{"529", errors.New("HTTP 529"), ErrorClassOverloaded},
		{"500", errors.New("HTTP 500"), ErrorClassServerError},
		{"502", errors.New("HTTP 502"), ErrorClassServerError},
		{"503", errors.New("HTTP 503"), ErrorClassServerError},
		{"504", errors.New("HTTP 504"), ErrorClassServerError},
		{"internal server error", errors.New("internal server error"), ErrorClassServerError},
		{"connection refused", errors.New("connection refused"), ErrorClassNetwork},
		{"timeout", errors.New("request timeout"), ErrorClassNetwork},
		{"eof", errors.New("unexpected EOF"), ErrorClassNetwork},
		{"reset", errors.New("connection reset"), ErrorClassNetwork},
		{"unknown", errors.New("something else"), ErrorClassUnknown},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			class, _ := ClassifyError(tt.err)
			if class != tt.wantClass {
				t.Errorf("ClassifyError(%v) = %d, want %d", tt.err, class, tt.wantClass)
			}
		})
	}
}

func TestIsRetryable(t *testing.T) {
	tests := []struct {
		class ErrorClass
		want  bool
	}{
		{ErrorClassUnknown, false},
		{ErrorClassContextOverflow, false},
		{ErrorClassRateLimit, true},
		{ErrorClassOverloaded, true},
		{ErrorClassServerError, true},
		{ErrorClassNetwork, true},
	}

	for _, tt := range tests {
		t.Run(strconv.Itoa(int(tt.class)), func(t *testing.T) {
			if got := IsRetryable(tt.class); got != tt.want {
				t.Errorf("IsRetryable(%d) = %v, want %v", tt.class, got, tt.want)
			}
		})
	}
}

func TestPolicy_Retry_Success(t *testing.T) {
	p := DefaultPolicy()
	calls := 0
	err := p.Retry(context.Background(), func() error {
		calls++
		if calls < 2 {
			return errors.New("connection error")
		}
		return nil
	})
	if err != nil {
		t.Errorf("expected nil error, got %v", err)
	}
	if calls != 2 {
		t.Errorf("expected 2 calls, got %d", calls)
	}
}

func TestPolicy_Retry_AllFail(t *testing.T) {
	p := &Policy{
		MaxAttempts:   3,
		InitialDelay:  1 * time.Millisecond,
		MaxDelay:      10 * time.Millisecond,
		BackoffFactor: 2.0,
	}

	calls := 0
	err := p.Retry(context.Background(), func() error {
		calls++
		return errors.New("connection error")
	})
	if err == nil {
		t.Error("expected error, got nil")
	}
	if calls != 3 {
		t.Errorf("expected 3 calls, got %d", calls)
	}
}

func TestPolicy_Retry_NonRetryable(t *testing.T) {
	p := DefaultPolicy()
	calls := 0
	err := p.Retry(context.Background(), func() error {
		calls++
		return errors.New("context overflow")
	})
	if err == nil {
		t.Error("expected error, got nil")
	}
	if calls != 1 {
		t.Errorf("expected 1 call (non-retryable), got %d", calls)
	}
}

func TestPolicy_Retry_ContextCanceled(t *testing.T) {
	p := &Policy{
		MaxAttempts:   5,
		InitialDelay:  100 * time.Millisecond,
		MaxDelay:      1 * time.Second,
		BackoffFactor: 2.0,
	}

	ctx, cancel := context.WithCancel(context.Background())
	calls := 0

	err := p.Retry(ctx, func() error {
		calls++
		cancel() // Cancel deterministically during the attempt
		return errors.New("connection error")
	})

	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if !errors.Is(err, context.Canceled) {
		t.Errorf("expected error to wrap context.Canceled, got %v", err)
	}
	if calls != 1 {
		t.Errorf("expected exactly 1 call, got %d", calls)
	}
}

func TestPolicy_RetryWithHeaders(t *testing.T) {
	p := DefaultPolicy()
	calls := 0
	err := p.RetryWithHeaders(context.Background(), func() (http.Header, error) {
		calls++
		if calls < 2 {
			return nil, errors.New("connection error")
		}
		return nil, nil
	})
	if err != nil {
		t.Errorf("expected nil error, got %v", err)
	}
	if calls != 2 {
		t.Errorf("expected 2 calls, got %d", calls)
	}
}

func TestParseRetryAfter_NegativeMs(t *testing.T) {
	headers := http.Header{}
	headers.Set("Retry-After-Ms", "-1")

	got := parseRetryAfter(headers)
	if got != 0 {
		t.Errorf("parseRetryAfter with negative ms = %v, want 0", got)
	}
}

func TestParseRetryAfter_ZeroMs(t *testing.T) {
	headers := http.Header{}
	headers.Set("Retry-After-Ms", "0")

	got := parseRetryAfter(headers)
	if got != 0 {
		t.Errorf("parseRetryAfter with zero ms = %v, want 0", got)
	}
}

func TestParseRetryAfter_EmptyHeaders(t *testing.T) {
	headers := http.Header{}
	got := parseRetryAfter(headers)
	if got != 0 {
		t.Errorf("parseRetryAfter with empty headers = %v, want 0", got)
	}

	got = parseRetryAfter(nil)
	if got != 0 {
		t.Errorf("parseRetryAfter(nil) = %v, want 0", got)
	}
}

func TestConfiguredPolicy(t *testing.T) {
	tests := []struct {
		name              string
		maxAttempts       int
		baseDelayMs       int
		maxDelayMs        int
		backoffMultiplier float64
		wantMaxAttempts   int
		wantInitialDelay  time.Duration
		wantMaxDelay      time.Duration
		wantBackoffFactor float64
	}{
		{
			name:              "all zeroes falls back to defaults",
			maxAttempts:       0,
			baseDelayMs:       0,
			maxDelayMs:        0,
			backoffMultiplier: 0,
			wantMaxAttempts:   3,
			wantInitialDelay:  1 * time.Second,
			wantMaxDelay:      30 * time.Second,
			wantBackoffFactor: 2.0,
		},
		{
			name:              "custom values are applied",
			maxAttempts:       5,
			baseDelayMs:       500,
			maxDelayMs:        5000,
			backoffMultiplier: 1.5,
			wantMaxAttempts:   5,
			wantInitialDelay:  500 * time.Millisecond,
			wantMaxDelay:      5000 * time.Millisecond,
			wantBackoffFactor: 1.5,
		},
		{
			name:              "negative values ignored (defaults kept)",
			maxAttempts:       -1,
			baseDelayMs:       -100,
			maxDelayMs:        -500,
			backoffMultiplier: -1.0,
			wantMaxAttempts:   3,
			wantInitialDelay:  1 * time.Second,
			wantMaxDelay:      30 * time.Second,
			wantBackoffFactor: 2.0,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := ConfiguredPolicy(tt.maxAttempts, tt.baseDelayMs, tt.maxDelayMs, tt.backoffMultiplier)
			if got.MaxAttempts != tt.wantMaxAttempts {
				t.Errorf("MaxAttempts = %d, want %d", got.MaxAttempts, tt.wantMaxAttempts)
			}
			if got.InitialDelay != tt.wantInitialDelay {
				t.Errorf("InitialDelay = %v, want %v", got.InitialDelay, tt.wantInitialDelay)
			}
			if got.MaxDelay != tt.wantMaxDelay {
				t.Errorf("MaxDelay = %v, want %v", got.MaxDelay, tt.wantMaxDelay)
			}
			if got.BackoffFactor != tt.wantBackoffFactor {
				t.Errorf("BackoffFactor = %f, want %f", got.BackoffFactor, tt.wantBackoffFactor)
			}
		})
	}
}
