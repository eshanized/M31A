package deps

import (
	"encoding/json"
	"testing"

	"github.com/eshanized/M31A/internal/core/config"
)

// testRules mirrors the safe defaults loaded from [intelligence.deps_risk].
func testRules() config.DepsRiskConfig {
	return config.DepsRiskConfig{
		StaleMonths:      12,
		YoungMonths:      6,
		LicenseAllowlist: []string{"MIT", "Apache-2.0", "BSD-3-Clause"},
	}
}

// cleanInput is a module that triggers nothing: allowlisted license,
// fresh release, live popular old repo, no instability signals.
func cleanInput() RiskInput {
	return RiskInput{
		HasVulns:              false,
		VulnCount:             0,
		LicenseSPDX:           "MIT",
		LatestReleaseAgeDays:  30, // released last month
		RepoArchived:          false,
		PopularityStars:       5000,
		ProjectAgeDays:        3650,
		APIInstabilitySignals: nil,
	}
}

func hasRule(rules []TriggeredRule, id string) *TriggeredRule {
	for i := range rules {
		if rules[i].RuleID == id {
			return &rules[i]
		}
	}
	return nil
}

func TestClassifyRiskVulnPresentIsHighFromOSV(t *testing.T) {
	in := cleanInput()
	in.HasVulns = true
	in.VulnCount = 2

	got := ClassifyRisk(in, testRules())
	if got.RiskClass != "high" {
		t.Fatalf("class = %q, want high for known vulnerability", got.RiskClass)
	}
	rule := hasRule(got.TriggeredRules, "vuln-present")
	if rule == nil {
		t.Fatal("vuln-present rule missing")
	}
	if rule.Severity != "high" || rule.Source != "osv" {
		t.Errorf("rule = %+v, want severity high sourced from osv", *rule)
	}
}

func TestClassifyRiskLicenseAllowlistCaseExact(t *testing.T) {
	in := cleanInput()
	got := ClassifyRisk(in, testRules())
	if hasRule(got.TriggeredRules, "license-external") != nil {
		t.Fatalf("MIT inside allowlist must not trigger: %+v", got)
	}

	// SPDX comparison is case-exact: "mit" is NOT "MIT" per DEPEND-02
	// ecosystem case rules.
	in2 := cleanInput()
	in2.LicenseSPDX = "mit"
	got2 := ClassifyRisk(in2, testRules())
	rule := hasRule(got2.TriggeredRules, "license-external")
	if rule == nil {
		t.Fatal("lowercase mit must trigger license-external (case-sensitive SPDX match)")
	}
	if got2.RiskClass != "high" || rule.Source != "depsdev" {
		t.Errorf("class/rule = %q/%+v, want high from depsdev", got2.RiskClass, *rule)
	}
}

func TestClassifyRiskStaleBoundaryGreaterOrEqual(t *testing.T) {
	staleDays := 12 * daysPerMonth // exact threshold

	below := cleanInput()
	below.LatestReleaseAgeDays = staleDays - 1
	gotBelow := ClassifyRisk(below, testRules())
	if hasRule(gotBelow.TriggeredRules, "release-stale") != nil {
		t.Fatalf("age %d below threshold must not trigger release-stale: %+v", staleDays-1, gotBelow)
	}

	exact := cleanInput()
	exact.LatestReleaseAgeDays = staleDays
	gotExact := ClassifyRisk(exact, testRules())
	rule := hasRule(gotExact.TriggeredRules, "release-stale")
	if rule == nil || gotExact.RiskClass != "high" {
		t.Fatalf("age exactly equal to stale_months must classify high (D-16 boundary): %+v", gotExact)
	}
	if rule.Severity != "high" || rule.Source != "depsdev" {
		t.Errorf("rule = %+v, want high from depsdev", *rule)
	}

	above := cleanInput()
	above.LatestReleaseAgeDays = staleDays + 1
	if gotAbove := ClassifyRisk(above, testRules()); gotAbove.RiskClass != "high" {
		t.Fatalf("age above threshold must be high, got %q", gotAbove.RiskClass)
	}
}

func TestClassifyRiskRepoArchivedHighFromGitHub(t *testing.T) {
	in := cleanInput()
	in.RepoArchived = true

	got := ClassifyRisk(in, testRules())
	if got.RiskClass != "high" {
		t.Fatalf("archived repo class = %q, want high", got.RiskClass)
	}
	rule := hasRule(got.TriggeredRules, "repo-archived")
	if rule == nil || rule.Source != "github" || rule.Severity != "high" {
		t.Fatalf("rule = %+v, want high from github", rule)
	}
}

func TestClassifyRiskPopularityYoungMedium(t *testing.T) {
	young := 6 * daysPerMonth

	lowAndYoung := cleanInput()
	lowAndYoung.PopularityStars = LowPopularityStars - 1
	lowAndYoung.ProjectAgeDays = young - 1
	got := ClassifyRisk(lowAndYoung, testRules())
	rule := hasRule(got.TriggeredRules, "popularity-young")
	if rule == nil || got.RiskClass != "medium" {
		t.Fatalf("low-popularity young module must be medium: %+v", got)
	}
	if rule.Severity != "medium" || rule.Source != "depsdev" {
		t.Errorf("rule = %+v, want medium from depsdev", *rule)
	}

	// Young but popular: no trigger.
	popular := cleanInput()
	popular.PopularityStars = 100000
	popular.ProjectAgeDays = young - 1
	if g := ClassifyRisk(popular, testRules()); hasRule(g.TriggeredRules, "popularity-young") != nil {
		t.Errorf("popular young module must not trigger: %+v", g)
	}

	// Old and unpopular: age gate blocks the rule (young_months bound).
	old := cleanInput()
	old.PopularityStars = LowPopularityStars - 1
	old.ProjectAgeDays = young + 1
	if g := ClassifyRisk(old, testRules()); hasRule(g.TriggeredRules, "popularity-young") != nil {
		t.Errorf("old unpopular module must not trigger popularity-young: %+v", g)
	}
}

func TestClassifyRiskAPIInstabilityMedium(t *testing.T) {
	in := cleanInput()
	in.APIInstabilitySignals = []string{"breaking-major-bump", "removed-symbols"}

	got := ClassifyRisk(in, testRules())
	rule := hasRule(got.TriggeredRules, "api-instability")
	if rule == nil || got.RiskClass != "medium" {
		t.Fatalf("instability signals must yield medium: %+v", got)
	}
	if rule.Source != "depsdev" {
		t.Errorf("source = %q, want depsdev", rule.Source)
	}
}

func TestClassifyRiskMultipleRulesCollapseToMaxSeverityAllListed(t *testing.T) {
	in := RiskInput{
		HasVulns:              true,
		VulnCount:             1,
		LicenseSPDX:           "GPL-3.0-only",
		LatestReleaseAgeDays:  13 * 30,
		RepoArchived:          false,
		PopularityStars:       1,
		ProjectAgeDays:        10,
		APIInstabilitySignals: []string{"unstable"},
	}

	got := ClassifyRisk(in, testRules())
	if got.RiskClass != "high" {
		t.Fatalf("class = %q, want high when any high-severity rule fires", got.RiskClass)
	}
	wantIDs := []string{"vuln-present", "license-external", "release-stale", "popularity-young", "api-instability"}
	if len(got.TriggeredRules) != len(wantIDs) {
		t.Fatalf("triggered rules = %d, want all %d listed", len(got.TriggeredRules), len(wantIDs))
	}
	for _, id := range wantIDs {
		if hasRule(got.TriggeredRules, id) == nil {
			t.Errorf("rule %s missing from %+v", id, got.TriggeredRules)
		}
	}

	// Medium-only combination collapses to medium, not low.
	medOnly := cleanInput()
	medOnly.APIInstabilitySignals = []string{"unstable"}
	medOnly.PopularityStars = 0
	medOnly.ProjectAgeDays = 5
	if g := ClassifyRisk(medOnly, testRules()); g.RiskClass != "medium" {
		t.Fatalf("medium-only class = %q, want medium", g.RiskClass)
	}
}

func TestClassifyRiskZeroRulesLowWithNonNilSlice(t *testing.T) {
	got := ClassifyRisk(cleanInput(), testRules())
	if got.RiskClass != "low" {
		t.Fatalf("clean input class = %q, want low", got.RiskClass)
	}
	if got.TriggeredRules == nil || len(got.TriggeredRules) != 0 {
		t.Fatalf("TriggeredRules = %#v, want empty-but-non-nil", got.TriggeredRules)
	}
}

func TestClassifyRiskDeterministicAcrossRuns(t *testing.T) {
	in := RiskInput{
		HasVulns:              true,
		VulnCount:             3,
		LicenseSPDX:           "Unknown",
		LatestReleaseAgeDays:  400,
		RepoArchived:          true,
		PopularityStars:       0,
		ProjectAgeDays:        2,
		APIInstabilitySignals: []string{"sig"},
	}
	first, second := ClassifyRisk(in, testRules()), ClassifyRisk(in, testRules())

	a, err := json.Marshal(first)
	if err != nil {
		t.Fatalf("marshal first: %v", err)
	}
	b, err := json.Marshal(second)
	if err != nil {
		t.Fatalf("marshal second: %v", err)
	}
	if string(a) != string(b) {
		t.Fatalf("assessments differ across identical runs:\n%s\n%s", a, b)
	}
}

func TestClassifyRiskEveryRuleCarriesProvenanceSource(t *testing.T) {
	in := RiskInput{
		HasVulns:              true,
		VulnCount:             1,
		LicenseSPDX:           "AGPL-3.0",
		LatestReleaseAgeDays:  24 * daysPerMonth,
		RepoArchived:          true,
		PopularityStars:       0,
		ProjectAgeDays:        1,
		APIInstabilitySignals: []string{"sig"},
	}
	got := ClassifyRisk(in, testRules())
	wantSources := map[string]string{
		"vuln-present":     "osv",
		"license-external": "depsdev",
		"release-stale":    "depsdev",
		"repo-archived":    "github",
		"api-instability":  "depsdev",
	}
	for id, source := range wantSources {
		rule := hasRule(got.TriggeredRules, id)
		if rule == nil {
			t.Errorf("rule %s missing", id)
			continue
		}
		if rule.Source != source {
			t.Errorf("rule %s source = %q, want %q (DEPEND-02 provenance)", id, rule.Source, source)
		}
		if rule.Detail == "" {
			t.Errorf("rule %s carries no detail", id)
		}
	}
}
