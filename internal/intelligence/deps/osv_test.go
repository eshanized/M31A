package deps

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	coreerrors "github.com/eshanized/M31A/internal/core/errors"
)

// captureOSVBody records the raw request body the server receives so tests
// can assert exact ecosystem/version normalization.
type capturedRequest struct {
	body    string
	header  http.Header
	path    string
	method  string
	capture func(r *http.Request)
}

func newCaptureServer(t *testing.T, status int, respond string) (*httptest.Server, *capturedRequest) {
	t.Helper()
	cap := &capturedRequest{}
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		b, _ := io.ReadAll(r.Body)
		cap.body = string(b)
		cap.header = r.Header.Clone()
		cap.path = r.URL.EscapedPath()
		cap.method = r.Method
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(status)
		fmt.Fprint(w, respond)
	}))
	return srv, cap
}

func TestOSVQueryVulnsOrderingSeverityDescendingThenID(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		fmt.Fprint(w, fixture(t, "osv_query.json"))
	}))
	defer srv.Close()

	q, err := NewOSVClient(srv.URL, srv.Client()).QueryVulns(context.Background(), "github.com/owner/repo", "v1.2.3")
	if err != nil {
		t.Fatalf("QueryVulns: %v", err)
	}
	if !q.QueriedOK {
		t.Fatal("QueriedOK = false on a successful query")
	}
	wantIDs := []string{"OSV-critical-first", "GHSA-high-middle", "CVE-low-late", "AAA-unknown-penult", "ZZZ-last"}
	if len(q.Vulns) != len(wantIDs) {
		t.Fatalf("got %d vulns, want %d", len(q.Vulns), len(wantIDs))
	}
	for i, want := range wantIDs {
		if q.Vulns[i].ID != want {
			t.Fatalf("vulns[%d].ID = %q, want %q (order: %v)", i, q.Vulns[i].ID, want, ids(q.Vulns))
		}
	}
}

func ids(vulns []Vuln) []string {
	out := make([]string, len(vulns))
	for i, v := range vulns {
		out[i] = v.ID
	}
	return out
}

func TestOSVQueryVulnsFixedInCollectedFromRanges(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		fmt.Fprint(w, fixture(t, "osv_query.json"))
	}))
	defer srv.Close()

	q, err := NewOSVClient(srv.URL, srv.Client()).QueryVulns(context.Background(), "github.com/owner/repo", "v1.2.3")
	if err != nil {
		t.Fatalf("QueryVulns: %v", err)
	}
	byID := map[string]Vuln{}
	for _, v := range q.Vulns {
		byID[v.ID] = v
	}
	if got := byID["OSV-critical-first"].FixedIn; len(got) != 1 || got[0] != "2.1.0" {
		t.Errorf("FixedIn = %v, want [2.1.0]", got)
	}
	if got := byID["GHSA-high-middle"].FixedIn; len(got) != 1 || got[0] != "1.25.0" {
		t.Errorf("FixedIn = %v, want [1.25.0]", got)
	}
	if got := byID["CVE-low-late"].Aliases; len(got) != 0 {
		t.Errorf("aliases on alias-free advisory = %v, want empty", got)
	}
}

func TestOSVVersionNormalizationDistinctBodies(t *testing.T) {
	var mu struct{ bodies []string }
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		b, _ := io.ReadAll(r.Body)
		mu.bodies = append(mu.bodies, string(b))
		fmt.Fprint(w, `{"vulns": []}`)
	}))
	defer srv.Close()
	c := NewOSVClient(srv.URL, srv.Client())

	for _, version := range []string{"1.2.3", "v2.0.0"} {
		if _, err := c.QueryVulns(context.Background(), "github.com/owner/repo", version); err != nil {
			t.Fatalf("QueryVulns(%q): %v", version, err)
		}
	}
	if len(mu.bodies) != 2 {
		t.Fatalf("%d requests recorded, want 2", len(mu.bodies))
	}

	var first, second struct {
		Package struct {
			Name      string `json:"name"`
			Ecosystem string `json:"ecosystem"`
		} `json:"package"`
		Version string `json:"version"`
	}
	if err := json.Unmarshal([]byte(mu.bodies[0]), &first); err != nil {
		t.Fatalf("decode body 1: %v", err)
	}
	if err := json.Unmarshal([]byte(mu.bodies[1]), &second); err != nil {
		t.Fatalf("decode body 2: %v", err)
	}
	// v-less input gains the Go-ecosystem prefix; already-prefixed unchanged.
	if first.Version != "v1.2.3" {
		t.Errorf("body1 version = %q, want v1.2.3 (prefix added)", first.Version)
	}
	if second.Version != "v2.0.0" {
		t.Errorf("body2 version = %q, want v2.0.0 (unchanged)", second.Version)
	}
	if first.Package.Ecosystem != "Go" || first.Package.Name != "github.com/owner/repo" {
		t.Errorf("body1 package = %+v, want ecosystem Go and module name verbatim", first.Package)
	}
}

func TestOSVBuildMetadataPreservedVerbatim(t *testing.T) {
	var seen []string
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		var body struct {
			Version string `json:"version"`
		}
		b, _ := io.ReadAll(r.Body)
		if err := json.Unmarshal(b, &body); err != nil {
			t.Errorf("decode request body: %v", err)
		}
		seen = append(seen, body.Version)
		fmt.Fprint(w, `{"vulns": []}`)
	}))
	defer srv.Close()
	c := NewOSVClient(srv.URL, srv.Client())

	for _, version := range []string{"v1.0.0", "v1.0.0+build1"} {
		if _, err := c.QueryVulns(context.Background(), "github.com/owner/repo", version); err != nil {
			t.Fatalf("QueryVulns(%q): %v", version, err)
		}
	}
	if len(seen) != 2 || seen[0] == seen[1] {
		t.Fatalf("versions seen = %v; build-metadata suffix must make a distinct lookup", seen)
	}
	if seen[1] != "v1.0.0+build1" {
		t.Fatalf("second version = %q, want v1.0.0+build1 preserved verbatim", seen[1])
	}
}

func TestOSVQueriedEmptyDistinctFromUnreachable(t *testing.T) {
	live := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		fmt.Fprint(w, `{"vulns": []}`)
	}))
	defer live.Close()

	qEmpty, err := NewOSVClient(live.URL, live.Client()).QueryVulns(context.Background(), "github.com/owner/repo", "v1.0.0")
	if err != nil {
		t.Fatalf("empty query errored: %v", err)
	}
	if !qEmpty.QueriedOK || len(qEmpty.Vulns) != 0 {
		t.Fatalf("queried-empty = %+v, want QueriedOK true with zero vulns", qEmpty)
	}

	dead := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {}))
	deadURL := dead.URL
	dead.Close() // unreachable now

	rec := &sleepRecorder{}
	c := NewOSVClient(deadURL, &http.Client{})
	c.api.sleep = rec.sleep
	c.api.baseDelay = time.Millisecond

	qDead, err := c.QueryVulns(context.Background(), "github.com/owner/repo", "v1.0.0")
	if !errors.Is(err, coreerrors.ErrSourceUnreachable) {
		t.Fatalf("err = %v, want wrapped ErrSourceUnreachable", err)
	}
	if qDead.QueriedOK || len(qDead.Vulns) != 0 {
		t.Fatalf("unreachable result = %+v, transport failure must never look like a clean result", qDead)
	}
}

func TestGitHubRepoActivityParsesArchivedAndCounts(t *testing.T) {
	srv, cap := newCaptureServer(t, http.StatusOK, fixture(t, "github_repo.json"))
	defer srv.Close()

	got, err := NewGitHubEnricher(srv.URL, srv.Client(), "").RepoActivity(context.Background(), "owner/repo")
	if err != nil {
		t.Fatalf("RepoActivity: %v", err)
	}
	if !got.Archived {
		t.Error("archived = false, want true")
	}
	if !got.PushedAt.Equal(time.Date(2026, 1, 15, 10, 30, 0, 0, time.UTC)) || got.PushedAt.Location() != time.UTC {
		t.Errorf("pushedAt = %v (%v), want 2026-01-15T10:30:00Z UTC", got.PushedAt, got.PushedAt.Location())
	}
	if got.OpenIssuesAndPRs != 42 {
		t.Errorf("OpenIssuesAndPRs = %d, want 42 (issues AND PRs per GitHub semantics)", got.OpenIssuesAndPRs)
	}
	if got.Stars != 1234 {
		t.Errorf("Stars = %d, want 1234", got.Stars)
	}
	if cap.path != "/repos/owner/repo" {
		t.Errorf("path = %q, want /repos/owner/repo", cap.path)
	}
}

func TestGitHubAuthHeaderPresenceFollowsToken(t *testing.T) {
	withToken, capTok := newCaptureServer(t, http.StatusOK, fixture(t, "github_repo.json"))
	defer withToken.Close()
	if _, err := NewGitHubEnricher(withToken.URL, withToken.Client(), "tok-123").RepoActivity(context.Background(), "owner/repo"); err != nil {
		t.Fatalf("RepoActivity(token): %v", err)
	}
	if got := capTok.header.Get("Authorization"); got != "Bearer tok-123" {
		t.Fatalf("Authorization = %q, want Bearer token attached when non-empty", got)
	}

	noToken, capNone := newCaptureServer(t, http.StatusOK, fixture(t, "github_repo.json"))
	defer noToken.Close()
	if _, err := NewGitHubEnricher(noToken.URL, noToken.Client(), "").RepoActivity(context.Background(), "owner/repo"); err != nil {
		t.Fatalf("RepoActivity(no token): %v", err)
	}
	if got := capNone.header.Get("Authorization"); got != "" {
		t.Fatalf("Authorization = %q, want absent header when token empty", got)
	}
}

func TestGitHubRateLimitMapsToTypedListedErrorWithHint(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("X-RateLimit-Remaining", "0")
		w.WriteHeader(http.StatusForbidden)
		fmt.Fprint(w, `{"message": "API rate limit exceeded"}`)
	}))
	defer srv.Close()

	_, err := NewGitHubEnricher(srv.URL, srv.Client(), "").RepoActivity(context.Background(), "owner/repo")
	if !errors.Is(err, coreerrors.ErrRateLimited) {
		t.Fatalf("err = %v, want typed rate-limit error", err)
	}
	if !strings.Contains(err.Error(), "GITHUB_TOKEN") {
		t.Fatalf("err = %v, hint must mention GITHUB_TOKEN", err)
	}
	if strings.Contains(err.Error(), "\n") && strings.Contains(err.Error(), "tok-") {
		t.Fatal("token material must never leak into errors")
	}
}

func TestGitHubRepoNotFoundSentinel(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusNotFound)
		fmt.Fprint(w, `{"message": "Not Found"}`)
	}))
	defer srv.Close()

	_, err := NewGitHubEnricher(srv.URL, srv.Client(), "").RepoActivity(context.Background(), "owner/gone")
	if !errors.Is(err, ErrPackageNotFound) {
		t.Fatalf("err = %v, want ErrPackageNotFound for missing repo", err)
	}
}

func TestRepoFromModuleHeuristics(t *testing.T) {
	cases := []struct {
		module string
		want   string
		fails  bool
	}{
		{module: "github.com/x/y", want: "x/y"},
		{module: "github.com/x/y/v2", want: "x/y"},
		{module: "github.com/x/y/subpkg", want: "x/y"},
		{module: "gitlab.com/x/y", fails: true},
		{module: "golang.org/x/tools", fails: true},
		{module: "github.com/x", fails: true},
	}
	for _, tc := range cases {
		got, err := RepoFromModule(tc.module)
		if tc.fails {
			if !errors.Is(err, ErrNotEnrichable) {
				t.Errorf("RepoFromModule(%q) err = %v, want ErrNotEnrichable", tc.module, err)
			}
			continue
		}
		if err != nil || got != tc.want {
			t.Errorf("RepoFromModule(%q) = %q, %v; want %q", tc.module, got, err, tc.want)
		}
	}
}
