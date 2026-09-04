package deps

import (
	"fmt"

	"github.com/eshanized/M31A/internal/core/config"
)

// daysPerMonth converts configured month thresholds into day ages. Ages
// enter classification as precomputed day counts so ClassifyRisk stays
// clock-free and deterministic.
const daysPerMonth = 30

// LowPopularityStars bounds "low popularity" for the D-16 young-package
// rule. The [intelligence.deps_risk] section carries no popularity knob,
// so this constant is the single documented definition; modules with at
// least this many stars are never flagged by popularity-young.
const LowPopularityStars = 10

// RiskInput aggregates client outputs plus precomputed values fed to
// ClassifyRisk. All ages are computed by the caller (UTC date math lives
// there); the classifier performs no I/O and reads no clock.
type RiskInput struct {
	HasVulns              bool
	VulnCount             int
	LicenseSPDX           string // SPDX identifier from deps.dev licenses[]; empty means unknown
	LatestReleaseAgeDays  int
	RepoArchived          bool
	PopularityStars       int
	ProjectAgeDays        int
	APIInstabilitySignals []string
}

// TriggeredRule records one fired D-16 rule with its severity, human-
// readable detail, and Source provenance naming the snapshot that fired
// it (osv | depsdev | github) per DEPEND-02.
type TriggeredRule struct {
	RuleID   string `json:"rule_id"`
	Severity string `json:"severity"` // high | medium
	Detail   string `json:"detail"`
	Source   string `json:"source"`
}

// RiskAssessment is the classified outcome: the collapsed class plus every
// triggered rule. TriggeredRules is always non-nil so JSON consumers see
// [] rather than null.
type RiskAssessment struct {
	RiskClass      string          `json:"risk_class"` // high | medium | low
	TriggeredRules []TriggeredRule `json:"triggered_rules"`
}

// ClassifyRisk applies the exact D-16 rule set against config-driven
// thresholds. Pure function: identical inputs yield byte-identical
// assessments; no I/O, no clock reads. Thresholds come from the caller's
// normalized config.Intelligence.DepsRisk (stale_months, young_months,
// license_allowlist), keeping D-16 adjustable without recompiling. Rules
// evaluate in fixed order; multiple hits collapse to the highest severity
// class while all rules stay listed.
//
// Boundary convention: release age greater-OR-EQUAL to stale_months is
// high (age exactly equal to the bound classifies high).
func ClassifyRisk(input RiskInput, rules config.DepsRiskConfig) RiskAssessment {
	staleDays := rules.StaleMonths * daysPerMonth
	youngDays := rules.YoungMonths * daysPerMonth

	// Fixed-order rule closures; nil return means not triggered.
	ruleSet := []func(RiskInput, config.DepsRiskConfig) *TriggeredRule{
		// known vulnerability present -> high (source: OSV)
		func(in RiskInput, _ config.DepsRiskConfig) *TriggeredRule {
			if !in.HasVulns || in.VulnCount <= 0 {
				return nil
			}
			return &TriggeredRule{RuleID: "vuln-present", Severity: "high",
				Detail: fmt.Sprintf("%d known vulnerabilities reported", in.VulnCount), Source: "osv"}
		},
		// license outside allowlist -> high (source: deps.dev)
		func(in RiskInput, cfg config.DepsRiskConfig) *TriggeredRule {
			if licenseAllowlisted(in.LicenseSPDX, cfg.LicenseAllowlist) {
				return nil
			}
			detail := fmt.Sprintf("license %q outside allowlist (case-sensitive SPDX match)", in.LicenseSPDX)
			if in.LicenseSPDX == "" {
				detail = "license unknown; treated as outside allowlist"
			}
			return &TriggeredRule{RuleID: "license-external", Severity: "high", Detail: detail, Source: "depsdev"}
		},
		// release age >= stale_months -> high (source: deps.dev)
		func(in RiskInput, _ config.DepsRiskConfig) *TriggeredRule {
			if in.LatestReleaseAgeDays < staleDays {
				return nil
			}
			return &TriggeredRule{RuleID: "release-stale", Severity: "high",
				Detail: fmt.Sprintf("latest release %d days old (threshold %d)", in.LatestReleaseAgeDays, staleDays),
				Source: "depsdev"}
		},
		// archived repo -> high (source: GitHub enrichment)
		func(in RiskInput, _ config.DepsRiskConfig) *TriggeredRule {
			if !in.RepoArchived {
				return nil
			}
			return &TriggeredRule{RuleID: "repo-archived", Severity: "high",
				Detail: "upstream repository archived", Source: "github"}
		},
		// low popularity AND project age under young_months -> medium (source: deps.dev)
		func(in RiskInput, _ config.DepsRiskConfig) *TriggeredRule {
			if in.PopularityStars >= LowPopularityStars || in.ProjectAgeDays >= youngDays {
				return nil
			}
			return &TriggeredRule{RuleID: "popularity-young", Severity: "medium",
				Detail: fmt.Sprintf("low popularity (%d stars) on young project (%d days old)", in.PopularityStars, in.ProjectAgeDays),
				Source: "depsdev"}
		},
		// API instability signals -> medium (source: deps.dev)
		func(in RiskInput, _ config.DepsRiskConfig) *TriggeredRule {
			if len(in.APIInstabilitySignals) == 0 {
				return nil
			}
			return &TriggeredRule{RuleID: "api-instability", Severity: "medium",
				Detail: fmt.Sprintf("API instability signals: %s", joinSignals(in.APIInstabilitySignals)), Source: "depsdev"}
		},
	}

	assessment := RiskAssessment{
		RiskClass:      "low",
		TriggeredRules: make([]TriggeredRule, 0, len(ruleSet)),
	}
	for _, eval := range ruleSet {
		if rule := eval(input, rules); rule != nil {
			assessment.TriggeredRules = append(assessment.TriggeredRules, *rule)
		}
	}

	// Collapse to highest severity; all rules remain listed.
	for _, rule := range assessment.TriggeredRules {
		if rule.Severity == "high" {
			assessment.RiskClass = "high"
			break
		}
		if rule.Severity == "medium" {
			assessment.RiskClass = "medium"
		}
	}
	return assessment
}

// licenseAllowlisted reports membership using case-exact SPDX comparison
// per DEPEND-02 ecosystem case rules: "MIT" matches only "MIT".
func licenseAllowlisted(spdx string, allowlist []string) bool {
	for _, allowed := range allowlist {
		if allowed == spdx {
			return true
		}
	}
	return false
}

// joinSignals renders instability signals deterministically.
func joinSignals(signals []string) string {
	out := ""
	for i, s := range signals {
		if i > 0 {
			out += ", "
		}
		out += s
	}
	return out
}
