// Package deps implements the dependency-intelligence data layer behind
// `m31a deps check` (D-13): fixture-testable clients for deps.dev v3,
// OSV.dev and GitHub repo enrichment, plus the config-driven risk
// classifier (D-16). Every client queries fixed hosts only, caps response
// bodies, validates required fields after decoding, and degrades with
// typed errors so verdict assembly can distinguish "queried clean" from
// "source unavailable" (DEPEND-02). Links found inside responses are data
// for display; nothing here ever dereferences them (SSRF guard).
package deps

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"

	coreerrors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
)

// ErrPackageNotFound reports that a registry answered 404 for a module or
// project lookup. Callers translate it into exists=false verdict data with
// cited sources instead of treating it as a failure.
var ErrPackageNotFound = errors.New("dependency package not found")

// errResponseBodyTooLarge reports a response body beyond the 2MB cap.
// Registry responses are untrusted input (T-04-06a); oversized bodies are
// rejected rather than decoded.
var errResponseBodyTooLarge = errors.New("registry response body exceeds size cap")

// maxRegistryResponseBytes caps every registry response body at 2MB via a
// io.LimitReader before any decoding happens.
const maxRegistryResponseBytes int64 = 2 << 20

// sleepFunc pauses between retry attempts. Injected by tests to observe
// retry timing without real wall-clock waits; the default honors ctx
// cancellation immediately.
type sleepFunc func(ctx context.Context, d time.Duration) error

// sleepContext waits d or until ctx is done, whichever comes first.
func sleepContext(ctx context.Context, d time.Duration) error {
	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-time.After(d):
		return nil
	}
}

// registryHTTPClient returns the hardened default client mirroring the
// provider layer's CatalogClient: bounded dial/TLS/header timeouts and a
// hard overall wall-clock timeout so a stalled registry can never hang a
// deps check indefinitely.
func registryHTTPClient() *http.Client {
	return &http.Client{
		Transport: &http.Transport{
			DialContext:           (&net.Dialer{Timeout: types.HTTPDialTimeout}).DialContext,
			TLSHandshakeTimeout:   10 * time.Second,
			ResponseHeaderTimeout: 10 * time.Second,
			MaxIdleConns:          100,
			MaxIdleConnsPerHost:   10,
			IdleConnTimeout:       90 * time.Second,
		},
		Timeout: types.FetchModelsTimeout,
	}
}

// registryClient carries the shared request plumbing for all three
// registry clients: BaseClient resilience semantics replicated locally
// (initial_only retry, 3 attempts, 1s base delay) so the intelligence
// layer never imports provider internals. Base URLs are injectable so
// tests run against httptest fixtures with zero network access.
type registryClient struct {
	baseURL     string
	client      *http.Client
	maxAttempts int
	baseDelay   time.Duration
	sleep       sleepFunc
	// onStatus, when set, inspects every non-nil response before generic
	// status mapping and may return a typed error that short-circuits
	// retry handling (used for GitHub's rate-limit signature).
	onStatus func(resp *http.Response) error
}

func newRegistryClient(baseURL string, httpClient *http.Client) *registryClient {
	if httpClient == nil {
		httpClient = registryHTTPClient()
	}
	return &registryClient{
		baseURL:     strings.TrimRight(baseURL, "/"),
		client:      httpClient,
		maxAttempts: 3,
		baseDelay:   time.Second,
		sleep:       sleepContext,
	}
}

// retryAfterDelay parses a Retry-After header value: delay seconds or an
// HTTP-date. A missing/unparseable header yields ok=false.
func retryAfterDelay(v string, now time.Time) (d time.Duration, ok bool) {
	if v == "" {
		return 0, false
	}
	if secs, err := strconv.Atoi(strings.TrimSpace(v)); err == nil {
		if secs < 0 {
			return 0, false
		}
		return time.Duration(secs) * time.Second, true
	}
	if when, err := http.ParseTime(v); err == nil {
		if delay := when.Sub(now); delay > 0 {
			return delay, true
		}
		return 0, true
	}
	return 0, false
}

// do executes method baseURL+path (with optional body and extra headers),
// retrying initial connection failures and honoring Retry-After on 429
// within the attempt budget. Non-retryable statuses map to typed errors:
// 404 becomes ErrPackageNotFound, everything else wraps the shared
// source-unreachable sentinel so callers can cap confidence (DEPEND-02).
func (r *registryClient) do(ctx context.Context, method, path string, body []byte, header http.Header) ([]byte, error) {
	var lastErr error
	for attempt := 0; attempt < r.maxAttempts; attempt++ {
		var reader io.Reader
		if body != nil {
			reader = strings.NewReader(string(body))
		}
		req, err := http.NewRequestWithContext(ctx, method, r.baseURL+path, reader)
		if err != nil {
			return nil, fmt.Errorf("build %s request: %w", method, err)
		}
		req.Header.Set("Accept", "application/json")
		for k, vals := range header {
			for _, v := range vals {
				req.Header.Add(k, v)
			}
		}

		resp, err := r.client.Do(req)
		if err != nil {
			// Transport-level failure: initial_only retries these with
			// exponential backoff, then surfaces unreachable (never a
			// fabricated clean result).
			lastErr = fmt.Errorf("%w: %s %s: %v", coreerrors.ErrSourceUnreachable, method, r.baseURL+path, err)
			if attempt >= r.maxAttempts-1 {
				break
			}
			if waitErr := r.sleep(ctx, r.baseDelay<<uint(attempt)); waitErr != nil {
				return nil, waitErr
			}
			continue
		}

		if r.onStatus != nil {
			if herr := r.onStatus(resp); herr != nil {
				_, _ = io.Copy(io.Discard, io.LimitReader(resp.Body, 4096))
				_ = resp.Body.Close()
				return nil, herr
			}
		}

		switch resp.StatusCode {
		case http.StatusOK:
			return readBodyLimited(resp)
		case http.StatusNotFound:
			_, _ = io.Copy(io.Discard, io.LimitReader(resp.Body, 4096))
			_ = resp.Body.Close()
			return nil, fmt.Errorf("%w: %s", ErrPackageNotFound, path)
		case http.StatusTooManyRequests:
			retryAfter := resp.Header.Get("Retry-After")
			_, _ = io.Copy(io.Discard, io.LimitReader(resp.Body, 4096))
			_ = resp.Body.Close()
			if attempt >= r.maxAttempts-1 {
				return nil, fmt.Errorf("%w: %s rate limited after %d attempts (retry-after: %s)", coreerrors.ErrRateLimited, r.baseURL+path, r.maxAttempts, retryAfter)
			}
			delay, ok := retryAfterDelay(retryAfter, time.Now())
			if !ok || delay <= 0 {
				delay = r.baseDelay
			}
			if waitErr := r.sleep(ctx, delay); waitErr != nil {
				return nil, waitErr
			}
			continue
		default:
			_, _ = io.Copy(io.Discard, io.LimitReader(resp.Body, 4096))
			_ = resp.Body.Close()
			return nil, fmt.Errorf("%w: %s %s returned status %d", coreerrors.ErrSourceUnreachable, method, r.baseURL+path, resp.StatusCode)
		}
	}
	return nil, lastErr
}

// readBodyLimited drains a 200 response through the 2MB LimitReader cap.
func readBodyLimited(resp *http.Response) ([]byte, error) {
	defer func() { _ = resp.Body.Close() }()
	body, err := io.ReadAll(io.LimitReader(resp.Body, maxRegistryResponseBytes+1))
	if err != nil {
		return nil, fmt.Errorf("read registry response: %w", err)
	}
	if int64(len(body)) > maxRegistryResponseBytes {
		return nil, fmt.Errorf("%w: %d bytes", errResponseBodyTooLarge, len(body))
	}
	return body, nil
}

// decodeJSON unmarshals into minimal structs; unknown fields are tolerated
// because third-party schemas evolve. Required-field validation happens in
// the caller, which names the offending field in its error.
func decodeJSON(data []byte, v interface{}) error {
	if err := json.Unmarshal(data, v); err != nil {
		return fmt.Errorf("decode registry response: %w", err)
	}
	return nil
}

// VersionKey identifies one package version in the deps.dev schema.
type VersionKey struct {
	System  string `json:"system"`
	Name    string `json:"name"`
	Version string `json:"version"`
}

// AdvisoryKey references a security advisory affecting a version.
type AdvisoryKey struct {
	ID string `json:"id"`
}

// VersionInfo mirrors the deps.dev version payload. Fields the package
// listing omits (licenses, deprecatedReason) decode as zero values — that
// tolerance is deliberate, schemas evolve.
type VersionInfo struct {
	VersionKey       VersionKey    `json:"versionKey"`
	PublishedAt      time.Time     `json:"publishedAt"`
	IsDefault        bool          `json:"isDefault"`
	IsDeprecated     bool          `json:"isDeprecated"`
	DeprecatedReason string        `json:"deprecatedReason"`
	Licenses         []string      `json:"licenses"`
	AdvisoryKeys     []AdvisoryKey `json:"advisoryKeys"`
}

// PackageInfo is the GET packages/{module} payload: the module's release list.
type PackageInfo struct {
	Versions []VersionInfo `json:"versions"`
}

// ProjectScorecard holds the OpenSSF Scorecard aggregate for a project.
type ProjectScorecard struct {
	OverallScore float64 `json:"overallScore"` // [0,10]
}

// ProjectInfo is the GET projects/{id} payload: upstream maintenance and
// popularity signals consumed by risk classification (D-16).
type ProjectInfo struct {
	OpenIssuesCount int              `json:"openIssuesCount"`
	StarsCount      int              `json:"starsCount"`
	ForksCount      int              `json:"forksCount"`
	License         string           `json:"license"`
	Description     string           `json:"description"`
	Homepage        string           `json:"homepage"`
	Scorecard       ProjectScorecard `json:"scorecard"`
}

// DepsDevClient queries the deps.dev v3 JSON API. Only the fixed default
// host is ever contacted; tests inject httptest servers via baseURL.
type DepsDevClient struct {
	api *registryClient
}

// NewDepsDevClient creates a client against baseURL (default production:
// https://api.deps.dev). A nil httpClient applies the shared hardened
// default mirroring CatalogClient settings.
func NewDepsDevClient(baseURL string, httpClient *http.Client) *DepsDevClient {
	return &DepsDevClient{api: newRegistryClient(baseURL, httpClient)}
}

// GetPackage returns the module's version list from
// GET /v3/systems/GO/packages/{module}. Module paths are percent-encoded
// preserving nested paths and /vN major suffixes. An empty versions array
// is valid; a version without its versionKey is a decode-time validation
// error naming the field. PublishedAt values normalize to UTC.
func (c *DepsDevClient) GetPackage(ctx context.Context, module string) (PackageInfo, error) {
	data, err := c.api.do(ctx, http.MethodGet, "/v3/systems/GO/packages/"+url.PathEscape(module), nil, nil)
	if err != nil {
		return PackageInfo{}, err
	}
	var out PackageInfo
	if err := decodeJSON(data, &out); err != nil {
		return PackageInfo{}, err
	}
	if out.Versions == nil {
		out.Versions = []VersionInfo{}
	}
	for i := range out.Versions {
		out.Versions[i].PublishedAt = out.Versions[i].PublishedAt.UTC()
		if out.Versions[i].VersionKey.Version == "" {
			return PackageInfo{}, fmt.Errorf("deps.dev package response: versions[%d].versionKey.version is required", i)
		}
	}
	return out, nil
}

// GetVersion returns per-version detail (licenses, advisory keys) from
// GET /v3/systems/GO/packages/{module}/versions/{version}.
func (c *DepsDevClient) GetVersion(ctx context.Context, module, version string) (VersionInfo, error) {
	path := "/v3/systems/GO/packages/" + url.PathEscape(module) + "/versions/" + url.PathEscape(version)
	data, err := c.api.do(ctx, http.MethodGet, path, nil, nil)
	if err != nil {
		return VersionInfo{}, err
	}
	var out VersionInfo
	if err := decodeJSON(data, &out); err != nil {
		return VersionInfo{}, err
	}
	out.PublishedAt = out.PublishedAt.UTC()
	if out.VersionKey.Version == "" {
		return VersionInfo{}, fmt.Errorf("deps.dev version response: versionKey.version is required")
	}
	return out, nil
}

// GetProject returns maintenance/popularity detail from
// GET /v3/projects/{projectID}; the id (host/owner/repo) is PathEscape'd
// like module paths.
func (c *DepsDevClient) GetProject(ctx context.Context, projectID string) (ProjectInfo, error) {
	data, err := c.api.do(ctx, http.MethodGet, "/v3/projects/"+url.PathEscape(projectID), nil, nil)
	if err != nil {
		return ProjectInfo{}, err
	}
	var out ProjectInfo
	if err := decodeJSON(data, &out); err != nil {
		return ProjectInfo{}, err
	}
	return out, nil
}
