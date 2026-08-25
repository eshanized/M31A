package deps

import (
	"context"
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	coreerrors "github.com/eshanized/M31A/internal/core/errors"
)

// sleepRecorder captures delays the client would have waited, standing in
// for real wall-clock sleeps so tests stay fast and deterministic.
type sleepRecorder struct {
	delays []time.Duration
}

func (s *sleepRecorder) sleep(ctx context.Context, d time.Duration) error {
	s.delays = append(s.delays, d)
	return nil
}

// fixture loads a recorded JSON fixture from testdata/.
func fixture(t *testing.T, name string) string {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("testdata", name))
	if err != nil {
		t.Fatalf("load fixture %s: %v", name, err)
	}
	return string(data)
}

// newTestDepsDevClient builds a client pointed at an httptest server with
// injected sleeps so retry timing is observable without waiting.
func newTestDepsDevClient(t *testing.T, srv *httptest.Server, rec *sleepRecorder) *DepsDevClient {
	t.Helper()
	c := NewDepsDevClient(srv.URL, srv.Client())
	c.api.sleep = rec.sleep
	c.api.baseDelay = time.Millisecond
	return c
}

func TestDepsDevGetPackageHappyPath(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		fmt.Fprint(w, fixture(t, "depsdev_package.json"))
	}))
	defer srv.Close()

	got, err := newTestDepsDevClient(t, srv, &sleepRecorder{}).GetPackage(context.Background(), "github.com/owner/repo")
	if err != nil {
		t.Fatalf("GetPackage: %v", err)
	}
	if len(got.Versions) != 2 {
		t.Fatalf("versions = %d, want 2", len(got.Versions))
	}
	v := got.Versions[0]
	if v.VersionKey.System != "GO" || v.VersionKey.Name != "github.com/owner/repo" || v.VersionKey.Version != "v1.2.3" {
		t.Errorf("versionKey = %+v, want GO/github.com/owner/repo/v1.2.3", v.VersionKey)
	}
	if !v.PublishedAt.Equal(time.Date(2024, 3, 1, 12, 0, 0, 0, time.UTC)) {
		t.Errorf("publishedAt = %v, want 2024-03-01T12:00:00Z", v.PublishedAt)
	}
	if v.PublishedAt.Location() != time.UTC {
		t.Errorf("publishedAt location = %v, want UTC", v.PublishedAt.Location())
	}
	if !v.IsDefault || v.IsDeprecated {
		t.Errorf("isDefault/isDeprecated = %v/%v, want true/false", v.IsDefault, v.IsDeprecated)
	}
	if len(v.Licenses) != 1 || v.Licenses[0] != "MIT" {
		t.Errorf("licenses = %v, want [MIT]", v.Licenses)
	}

	dep := got.Versions[1]
	if !dep.IsDeprecated || dep.DeprecatedReason != "superseded by v1.2.3" {
		t.Errorf("deprecated fields = %v/%q", dep.IsDeprecated, dep.DeprecatedReason)
	}
	if len(dep.AdvisoryKeys) != 1 || dep.AdvisoryKeys[0].ID != "GHSA-aaaa-bbbb-cccc" {
		t.Errorf("advisoryKeys = %v, want single GHSA id", dep.AdvisoryKeys)
	}
	if dep.Licenses != nil {
		t.Errorf("licenses on version without the key = %v, want nil (unknown tolerated)", dep.Licenses)
	}
}

func TestDepsDevGetPackageEscapesModulePath(t *testing.T) {
	var gotPath string
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotPath = r.URL.EscapedPath()
		fmt.Fprint(w, fixture(t, "depsdev_package.json"))
	}))
	defer srv.Close()

	_, err := newTestDepsDevClient(t, srv, &sleepRecorder{}).GetPackage(context.Background(), "github.com/owner/repo/v2")
	if err != nil {
		t.Fatalf("GetPackage: %v", err)
	}
	// Nested paths and /v2 major suffixes must survive as ONE escaped path
	// segment; if slashes leaked through unescaped the server would see
	// extra path elements instead of a single %2F-encoded module.
	want := "/v3/systems/GO/packages/github.com%2Fowner%2Frepo%2Fv2"
	if gotPath != want {
		t.Fatalf("server saw path %q, want %q", gotPath, want)
	}
	if strings.Count(gotPath, "%2F") != 3 {
		t.Fatalf("expected 3 escaped separators in %q", gotPath)
	}
}

func TestDepsDevGetPackageNotFoundSentinel(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusNotFound)
		fmt.Fprint(w, `{"error": "not found"}`)
	}))
	defer srv.Close()

	_, err := newTestDepsDevClient(t, srv, &sleepRecorder{}).GetPackage(context.Background(), "github.com/owner/missing")
	if !errors.Is(err, ErrPackageNotFound) {
		t.Fatalf("err = %v, want ErrPackageNotFound sentinel", err)
	}
}

func TestDepsDevGetPackageRetryAfterOn429(t *testing.T) {
	hits := 0
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		hits++
		if hits == 1 {
			w.Header().Set("Retry-After", "1")
			w.WriteHeader(http.StatusTooManyRequests)
			return
		}
		fmt.Fprint(w, fixture(t, "depsdev_package.json"))
	}))
	defer srv.Close()
	rec := &sleepRecorder{}

	got, err := newTestDepsDevClient(t, srv, rec).GetPackage(context.Background(), "github.com/owner/repo")
	if err != nil {
		t.Fatalf("GetPackage: %v", err)
	}
	if hits != 2 {
		t.Fatalf("handler hits = %d, want exactly 2 attempts", hits)
	}
	if len(rec.delays) != 1 || rec.delays[0] != time.Second {
		t.Fatalf("sleeps = %v, want exactly the advertised Retry-After delay [1s]", rec.delays)
	}
	if len(got.Versions) != 2 {
		t.Fatalf("versions after retry = %d, want 2", len(got.Versions))
	}
}

func TestDepsDevGetPackageOversizedResponseCapped(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		// One byte beyond the cap must trip the typed error.
		if _, err := w.Write(make([]byte, maxRegistryResponseBytes+1)); err != nil {
			t.Errorf("write oversized body: %v", err)
		}
	}))
	defer srv.Close()

	_, err := newTestDepsDevClient(t, srv, &sleepRecorder{}).GetPackage(context.Background(), "github.com/owner/repo")
	if !errors.Is(err, errResponseBodyTooLarge) {
		t.Fatalf("err = %v, want errResponseBodyTooLarge", err)
	}
}

func TestDepsDevGetPackageMissingVersionKeyRejected(t *testing.T) {
	body := `{"versions": [{"publishedAt": "2024-03-01T12:00:00Z"}]}`
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		fmt.Fprint(w, body)
	}))
	defer srv.Close()

	_, err := newTestDepsDevClient(t, srv, &sleepRecorder{}).GetPackage(context.Background(), "github.com/owner/repo")
	if err == nil {
		t.Fatal("missing versionKey accepted")
	}
	if !strings.Contains(err.Error(), "versionKey") {
		t.Fatalf("err = %v, want field name versionKey named", err)
	}
}

func TestDepsDevGetPackageEmptyVersionsValid(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		fmt.Fprint(w, `{"versions": []}`)
	}))
	defer srv.Close()

	got, err := newTestDepsDevClient(t, srv, &sleepRecorder{}).GetPackage(context.Background(), "github.com/owner/empty")
	if err != nil {
		t.Fatalf("empty versions array rejected: %v", err)
	}
	if got.Versions == nil || len(got.Versions) != 0 {
		t.Fatalf("versions = %#v, want non-nil empty slice", got.Versions)
	}
}

func TestDepsDevTransportFailureRetriesWithinBudget(t *testing.T) {
	// Closed listener: every attempt fails at the transport layer,
	// exercising initial_only retry semantics to exhaustion.
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {}))
	base := srv.URL
	srv.Close() // now guaranteed unreachable

	rec := &sleepRecorder{}
	c := NewDepsDevClient(base, &http.Client{})
	c.api.sleep = rec.sleep
	c.api.baseDelay = time.Millisecond

	_, err := c.GetPackage(context.Background(), "github.com/owner/repo")
	if !errors.Is(err, coreerrors.ErrSourceUnreachable) {
		t.Fatalf("err = %v, want wrapped source-unreachable sentinel", err)
	}
	// 3 attempts => exactly 2 inter-attempt backoffs.
	if len(rec.delays) != 2 {
		t.Fatalf("sleeps = %v, want 2 backoff delays across 3 attempts", rec.delays)
	}
}

func TestDepsDevRetryHonorsContextCancellation(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusTooManyRequests)
	}))
	defer srv.Close()

	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	c := NewDepsDevClient(srv.URL, srv.Client())
	c.api.sleep = func(ctx context.Context, d time.Duration) error { return sleepContext(ctx, d) }

	if _, err := c.GetPackage(ctx, "github.com/owner/repo"); !errors.Is(err, context.Canceled) {
		t.Fatalf("err = %v, want context.Canceled honored during retry wait", err)
	}
}

func TestDepsDevGetVersionHappyPath(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !strings.HasSuffix(r.URL.EscapedPath(), "/versions/v1.2.3") {
			t.Errorf("unexpected path %q", r.URL.EscapedPath())
		}
		fmt.Fprint(w, fixture(t, "depsdev_version.json"))
	}))
	defer srv.Close()

	got, err := newTestDepsDevClient(t, srv, &sleepRecorder{}).GetVersion(context.Background(), "github.com/owner/repo", "v1.2.3")
	if err != nil {
		t.Fatalf("GetVersion: %v", err)
	}
	if got.VersionKey.Version != "v1.2.3" {
		t.Errorf("version = %q, want v1.2.3", got.VersionKey.Version)
	}
	if len(got.Licenses) != 1 || got.Licenses[0] != "Apache-2.0" {
		t.Errorf("licenses = %v, want [Apache-2.0]", got.Licenses)
	}
	if len(got.AdvisoryKeys) != 1 || got.AdvisoryKeys[0].ID != "GHSA-dddd-eeee-ffff" {
		t.Errorf("advisoryKeys = %v", got.AdvisoryKeys)
	}
	if got.PublishedAt.Location() != time.UTC {
		t.Errorf("publishedAt not normalized to UTC: %v", got.PublishedAt.Location())
	}
}

func TestDepsDevGetProjectHappyPath(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if got := r.URL.EscapedPath(); got != "/v3/projects/github.com%2Fowner%2Frepo" {
			t.Errorf("project id must be PathEscape'd, server saw %q", got)
		}
		fmt.Fprint(w, fixture(t, "depsdev_project.json"))
	}))
	defer srv.Close()

	got, err := newTestDepsDevClient(t, srv, &sleepRecorder{}).GetProject(context.Background(), "github.com/owner/repo")
	if err != nil {
		t.Fatalf("GetProject: %v", err)
	}
	if got.OpenIssuesCount != 12 || got.StarsCount != 3400 || got.ForksCount != 210 {
		t.Errorf("counts = %d/%d/%d, want 12/3400/210", got.OpenIssuesCount, got.StarsCount, got.ForksCount)
	}
	if got.License != "MIT" || got.Description != "example module" || got.Homepage != "https://example.com" {
		t.Errorf("text fields = %q/%q/%q", got.License, got.Description, got.Homepage)
	}
	if got.Scorecard.OverallScore != 7.5 {
		t.Errorf("scorecard.overallScore = %v, want 7.5", got.Scorecard.OverallScore)
	}
}
