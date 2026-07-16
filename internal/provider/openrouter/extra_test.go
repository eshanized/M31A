package openrouter

import (
	"context"
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/testutil"
	"github.com/eshanized/M31A/internal/types"
)

func TestIsRetryable_Nil(t *testing.T) {
	t.Parallel()
	if provider.IsRetryable(nil) {
		t.Error("IsRetryable(nil) should be false")
	}
}

func TestIsRetryable_500(t *testing.T) {
	t.Parallel()
	if !provider.IsRetryable(&provider.HTTPStatusError{StatusCode: 500, Message: "Internal Server Error"}) {
		t.Error("should retry on 500")
	}
}

func TestIsRetryable_502(t *testing.T) {
	t.Parallel()
	if !provider.IsRetryable(&provider.HTTPStatusError{StatusCode: 502, Message: "Bad Gateway"}) {
		t.Error("should retry on 502")
	}
}

func TestIsRetryable_503(t *testing.T) {
	t.Parallel()
	if !provider.IsRetryable(&provider.HTTPStatusError{StatusCode: 503, Message: "Service Unavailable"}) {
		t.Error("should retry on 503")
	}
}

func TestIsRetryable_ConnectionReset(t *testing.T) {
	t.Parallel()
	if !provider.IsRetryable(errors.New("connection reset by peer")) {
		t.Error("should retry on connection reset")
	}
}

func TestIsRetryable_UnexpectedEOF(t *testing.T) {
	t.Parallel()
	if !provider.IsRetryable(errors.New("unexpected EOF")) {
		t.Error("should retry on unexpected EOF")
	}
}

func TestIsRetryable_ServerError(t *testing.T) {
	t.Parallel()
	if !provider.IsRetryable(errors.New("server error")) {
		t.Error("should retry on server error")
	}
}

func TestIsRetryable_GatewayError(t *testing.T) {
	t.Parallel()
	if !provider.IsRetryable(errors.New("bad gateway error")) {
		t.Error("should retry on gateway error")
	}
}

func TestIsRetryable_TemporarilyUnavailable(t *testing.T) {
	t.Parallel()
	if !provider.IsRetryable(errors.New("service temporarily unavailable")) {
		t.Error("should retry on temporarily unavailable")
	}
}

func TestIsRetryable_NotRetryable(t *testing.T) {
	t.Parallel()
	if provider.IsRetryable(errors.New("invalid API key")) {
		t.Error("should not retry on invalid API key")
	}
}

func TestIsRetryable_WrappedError(t *testing.T) {
	t.Parallel()
	inner := errors.New("connection reset by peer")
	wrapped := fmt.Errorf("send request: %w", inner)
	if !provider.IsRetryable(wrapped) {
		t.Error("should retry on wrapped connection reset")
	}
}

func TestHealthCheck_Degraded(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		time.Sleep(200 * time.Millisecond)
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"data": {"label": "test"}}`))
	}))
	defer srv.Close()

	c, err := New("sk-or-v1-testkey", Options{
		BaseURL:           srv.URL,
		HealthCheckLiveMs: 10,
		HealthCheckSlowMs: 50,
		CacheTTL:          time.Minute,
		CacheStaleTTL:     time.Minute,
	})
	if err != nil {
		t.Fatal(err)
	}
	got := c.HealthCheck(context.Background())
	if got.Status != types.HealthStatusDegraded {
		t.Errorf("status = %q, want degraded (latency=%dms)", got.Status, got.LatencyMs)
	}
}

func TestHealthCheck_ContextCanceled(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		time.Sleep(5 * time.Second)
	}))
	defer srv.Close()

	c, err := New("sk-or-v1-testkey", Options{
		BaseURL:           srv.URL,
		HealthCheckLiveMs: 10,
		HealthCheckSlowMs: 100,
		CacheTTL:          time.Minute,
		CacheStaleTTL:     time.Minute,
	})
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 50*time.Millisecond)
	defer cancel()
	got := c.HealthCheck(ctx)
	if got.Status != types.HealthStatusOffline {
		t.Errorf("status = %q, want offline for context timeout", got.Status)
	}
}

func TestNew_ShortKey(t *testing.T) {
	t.Parallel()
	c, err := New("short", Options{})
	if err != nil {
		t.Fatal(err)
	}
	masked := c.APIKey()
	// Short keys show full content after "****" prefix
	if !strings.HasPrefix(masked, "****") {
		t.Errorf("short key masked = %q, should start with ****", masked)
	}
}

func TestOptions_Defaults(t *testing.T) {
	t.Parallel()
	testutil.LoadTestDotEnv(t)

	apiKey := os.Getenv("OPENROUTER_API_KEY")
	if apiKey == "" {
		apiKey = "sk-or-v1-testkey1234567890"
	}
	c, err := New(apiKey, Options{})
	if err != nil {
		t.Fatal(err)
	}
	if c.Name() != "openrouter" {
		t.Errorf("Name = %q", c.Name())
	}
}

func TestFetchModels_NetworkError(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusInternalServerError)
	}))
	defer srv.Close()

	c, err := New("sk-or-v1-testkey", Options{
		BaseURL:       srv.URL,
		CacheTTL:      time.Minute,
		CacheStaleTTL: time.Minute,
	})
	if err != nil {
		t.Fatal(err)
	}
	_, err = c.FetchModels(context.Background())
	if err == nil {
		t.Error("FetchModels should fail on 500")
	}
}

func TestChatCompletionStream_402(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusPaymentRequired)
		w.Write([]byte(`{"error": {"message": "insufficient credits"}}`))
	}))
	defer srv.Close()

	c, err := New("sk-or-v1-testkey", Options{
		BaseURL:       srv.URL,
		CacheTTL:      time.Minute,
		CacheStaleTTL: time.Minute,
	})
	if err != nil {
		t.Fatal(err)
	}
	_, err = c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "test-model",
		Messages: []types.Message{{Role: "user", Content: "hi"}},
	})
	if err == nil {
		t.Error("ChatCompletionStream should fail on 402")
	}
}
