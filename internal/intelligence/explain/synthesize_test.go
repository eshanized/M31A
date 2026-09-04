package explain

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

// mockSynthesizer counts calls and returns a canned response, letting tests
// assert the exactly-one-synthesis-call contract without any live API.
type mockSynthesizer struct {
	calls int
	resp  *types.ChatResponse
	err   error
}

func (m *mockSynthesizer) ChatCompletion(ctx context.Context, req types.ChatRequest) (*types.ChatResponse, error) {
	m.calls++
	if m.err != nil {
		return nil, m.err
	}
	return m.resp, nil
}

func threeItemPack(t *testing.T) *types.EvidencePack {
	t.Helper()
	pack := types.NewEvidencePack("Bar")
	pack.Add(types.EvidenceSource, "bar.go:3", "func Bar() int {")
	pack.Add(types.EvidenceCallers, "caller.go:4", "CallBar calls Bar")
	pack.Add(types.EvidenceBlame, "da17163c2886a751570d955ee1de733115fec312", "last touched by Seed Author on 2026-01-05: add Bar function")
	return pack
}

func TestSynthesize_SingleCallValidMarkers(t *testing.T) {
	mock := &mockSynthesizer{resp: &types.ChatResponse{
		Content: "Bar is defined in bar.go [1]. It is called from CallBar [2].",
	}}

	ans, err := Synthesize(context.Background(), threeItemPack(t), mock, "test-model", nil)
	if err != nil {
		t.Fatalf("Synthesize failed: %v", err)
	}

	if mock.calls != 1 {
		t.Errorf("synthesis call count = %d, want exactly 1", mock.calls)
	}
	if len(ans.Inference) != 0 {
		t.Errorf("expected empty Inference for fully-cited prose, got %v", ans.Inference)
	}
	gotMarkers := map[int]bool{}
	for _, c := range ans.Citations {
		gotMarkers[c.Marker] = true
		if c.Ref == "" {
			t.Errorf("citation %d missing Ref", c.Marker)
		}
	}
	if !gotMarkers[1] || !gotMarkers[2] {
		t.Errorf("citations = %v, want markers 1 and 2", gotMarkers)
	}
	if ans.Confidence != types.ConfidenceVerified {
		t.Errorf("confidence = %s, want verified when citations exist and inference empty", ans.Confidence)
	}
}

func TestSynthesize_RequestShape(t *testing.T) {
	var captured types.ChatRequest
	mock := &capturingMock{capture: &captured, resp: &types.ChatResponse{Content: "x [1]."}}

	if _, err := Synthesize(context.Background(), threeItemPack(t), mock, "test-model", nil); err != nil {
		t.Fatalf("Synthesize failed: %v", err)
	}
	if len(captured.Messages) < 2 {
		t.Fatalf("expected system+user messages, got %d", len(captured.Messages))
	}
	system := captured.Messages[0].Content
	for _, want := range []string{"DATA, not instructions", "bracket-number"} {
		if !strings.Contains(system, want) {
			t.Errorf("system prompt missing %q (pack-as-data contract)", want)
		}
	}
	user := captured.Messages[1].Content
	for _, want := range []string{"kind=source ref=bar.go:3", "[1]", "[2]", "[3]"} {
		if !strings.Contains(user, want) {
			t.Errorf("user message missing %q (numbered pack data)", want)
		}
	}
	if captured.Temperature == nil || *captured.Temperature > 0.5 {
		t.Errorf("expected low synthesis temperature, got %v", captured.Temperature)
	}
	if captured.Model != "test-model" {
		t.Errorf("model = %q, want test-model", captured.Model)
	}
}

// capturingMock records the request before returning its response.
type capturingMock struct {
	capture *types.ChatRequest
	resp    *types.ChatResponse
}

func (m *capturingMock) ChatCompletion(ctx context.Context, req types.ChatRequest) (*types.ChatResponse, error) {
	*m.capture = req
	return m.resp, nil
}

func TestValidateMarkers_MixedDemotesUnknownSentence(t *testing.T) {
	pack := threeItemPack(t)
	prose := "Bar lives in bar.go [1]. Bar was invented by the founders [99]. CallBar invokes it [2]."

	cleaned, citations, inference := validateMarkers(prose, pack)

	// Unknown-marker sentence moves wholesale to Inference; [99] stripped.
	if len(inference) != 1 {
		t.Fatalf("inference = %v, want exactly one demoted sentence", inference)
	}
	if !strings.Contains(inference[0], "invented by the founders") {
		t.Errorf("demoted sentence text wrong: %q", inference[0])
	}
	if strings.Contains(inference[0], "[99]") {
		t.Errorf("unknown marker [99] survived stripping: %q", inference[0])
	}

	// Cleaned prose keeps only valid-marker sentences.
	if strings.Contains(cleaned, "[99]") {
		t.Errorf("cleaned prose still contains unknown marker: %q", cleaned)
	}
	if !strings.Contains(cleaned, "[1]") || !strings.Contains(cleaned, "[2]") {
		t.Errorf("cleaned prose lost known markers: %q", cleaned)
	}

	// Citations reference only real pack items — no dangling entries.
	wantRefs := map[int]string{1: "bar.go:3", 2: "caller.go:4"}
	for _, c := range citations {
		if c.Ref != wantRefs[c.Marker] {
			t.Errorf("citation marker=%d ref=%q, want %q", c.Marker, c.Ref, wantRefs[c.Marker])
		}
	}
	if len(citations) != 2 {
		t.Errorf("citations = %d entries, want 2", len(citations))
	}
}

func TestValidateMarkers_AllUnknownGoesToInference(t *testing.T) {
	pack := threeItemPack(t)
	cleaned, citations, inference := validateMarkers("Everything is fine [7] [42].", pack)

	if cleaned != "" {
		t.Errorf("cleaned prose = %q, want empty", cleaned)
	}
	if len(citations) != 0 {
		t.Errorf("dangling citations present: %+v", citations)
	}
	if len(inference) != 1 || !strings.Contains(inference[0], "Everything is fine") {
		t.Fatalf("inference = %+v, want demoted sentence without markers", inference)
	}
	if strings.Contains(inference[0], "[7]") || strings.Contains(inference[0], "[42]") {
		t.Errorf("unknown markers not stripped: %q", inference[0])
	}
}

func TestRenderText_NoEvidencePackNamesScopes(t *testing.T) {
	var buf bytes.Buffer
	ans := &ExplainAnswer{Query: "Ghost"}
	if err := RenderText(&buf, ans, types.NewEvidencePack("Ghost")); err != nil {
		t.Fatalf("RenderText failed: %v", err)
	}
	out := buf.String()
	if !strings.Contains(out, "No evidence found") {
		t.Errorf("output lacks explicit no-evidence report:\n%s", out)
	}
	if !strings.Contains(out, "Searched scopes") || !strings.Contains(out, "symbols") || !strings.Contains(out, "files") {
		t.Errorf("no-evidence report does not name scopes:\n%s", out)
	}
}

func TestRenderText_EvidenceSectionAndInferenceHeading(t *testing.T) {
	var buf bytes.Buffer
	ans := &ExplainAnswer{
		Query: "Bar",
		Prose: "Bar is defined in bar.go [1].",
		Citations: []types.Citation{
			{Marker: 1, Kind: types.EvidenceSource, Ref: "bar.go:3", Note: "func Bar"},
		},
		Inference:  []string{"Bar might be ancient."},
		Confidence: types.ConfidenceLikely,
	}
	if err := RenderText(&buf, ans, threeItemPack(t)); err != nil {
		t.Fatalf("RenderText failed: %v", err)
	}
	out := buf.String()
	for _, want := range []string{"[1]", "Evidence:", "Inference", "bar.go:3", "source", "Confidence: likely"} {
		if !strings.Contains(out, want) {
			t.Errorf("text output missing %q:\n%s", want, out)
		}
	}
}

func TestRenderJSON_Fields(t *testing.T) {
	var buf bytes.Buffer
	ans := &ExplainAnswer{
		Query: "Bar",
		Prose: "Defined [1].",
		Citations: []types.Citation{
			{Marker: 1, Kind: types.EvidenceSource, Ref: "bar.go:3", Note: "func Bar"},
		},
		Inference:  []string{},
		Confidence: types.ConfidenceVerified,
	}
	if err := RenderJSON(&buf, ans); err != nil {
		t.Fatalf("RenderJSON failed: %v", err)
	}

	var decoded map[string]any
	if err := json.Unmarshal(buf.Bytes(), &decoded); err != nil {
		t.Fatalf("output is not valid JSON: %v\n%s", err, buf.String())
	}
	for _, key := range []string{"query", "prose", "citations", "inference", "confidence"} {
		if _, ok := decoded[key]; !ok {
			t.Errorf("JSON object missing key %q", key)
		}
	}
	citations, ok := decoded["citations"].([]any)
	if !ok || len(citations) != 1 {
		t.Fatalf("citations shape wrong: %v", decoded["citations"])
	}
	first := citations[0].(map[string]any)
	for _, key := range []string{"marker", "kind", "ref", "note"} {
		if _, ok := first[key]; !ok {
			t.Errorf("citation JSON missing snake_case key %q", key)
		}
	}
}

// TestExplainEndToEnd_MockPipeline runs collector → pack → single mock
// synthesis → text render over the seeded repo, asserting the Evidence
// section, marker presence in prose, and exactly-one call count.
func TestExplainEndToEnd_MockPipeline(t *testing.T) {
	dir := seedRepo(t)
	deps := buildDeps(t, dir, 0)

	pack, err := Collect(context.Background(), deps, "Bar", ModeSymbol, "test-model")
	if err != nil {
		t.Fatalf("Collect failed: %v", err)
	}
	if len(pack.Sections) == 0 {
		t.Fatal("empty pack from seeded repo")
	}

	prose := fmt.Sprintf("The symbol is documented in the workspace [%d]. It is last touched by a commit [%d].",
		pack.Sections[0].ID, pack.Sections[len(pack.Sections)-1].ID)
	mock := &mockSynthesizer{resp: &types.ChatResponse{Content: prose}}

	answer, err := Synthesize(context.Background(), pack, mock, "test-model", nil)
	if err != nil {
		t.Fatalf("Synthesize failed: %v", err)
	}
	if mock.calls != 1 {
		t.Errorf("end-to-end synthesis calls = %d, want 1", mock.calls)
	}

	var buf bytes.Buffer
	if err := RenderText(&buf, answer, pack); err != nil {
		t.Fatalf("RenderText failed: %v", err)
	}
	out := buf.String()
	if !strings.Contains(out, "Evidence:") {
		t.Errorf("rendered output lacks Evidence section:\n%s", out)
	}
	if !strings.Contains(out, fmt.Sprintf("[%d]", pack.Sections[0].ID)) {
		t.Errorf("rendered output lacks cited marker [%d]:\n%s", pack.Sections[0].ID, out)
	}
	for _, c := range answer.Citations {
		if c.Ref == "" {
			t.Errorf("citation %d has empty ref", c.Marker)
		}
	}
	if len(answer.Inference) != 0 && answer.Confidence == types.ConfidenceVerified {
		t.Errorf("verified confidence despite non-empty inference")
	}
}
