package decision

import (
	"sort"
	"strings"
	"time"
)

// Query specifies filters for searching decision logs.
type Query struct {
	Since      time.Time
	Until      time.Time
	Categories []Category
	Term       string
	Limit      int
}

// Find returns decisions matching the query, sorted by timestamp ascending.
func Find(decisions []DecisionReceipt, q Query) []DecisionReceipt {
	var result []DecisionReceipt
	for _, d := range decisions {
		if !q.Since.IsZero() && d.Timestamp.Before(q.Since) {
			continue
		}
		if !q.Until.IsZero() && d.Timestamp.After(q.Until) {
			continue
		}
		if len(q.Categories) > 0 && !matchesCategory(d.Category, q.Categories) {
			continue
		}
		if q.Term != "" && !matchesTerm(d, q.Term) {
			continue
		}
		result = append(result, d)
	}

	sort.Slice(result, func(i, j int) bool {
		return result[i].Timestamp.Before(result[j].Timestamp)
	})

	if q.Limit > 0 && len(result) > q.Limit {
		result = result[len(result)-q.Limit:]
	}
	return result
}

// ByCategory groups decisions by category.
func ByCategory(decisions []DecisionReceipt) map[Category][]DecisionReceipt {
	groups := make(map[Category][]DecisionReceipt)
	for _, d := range decisions {
		groups[d.Category] = append(groups[d.Category], d)
	}
	return groups
}

// CostSummary aggregates cost across a set of decisions.
func CostSummary(decisions []DecisionReceipt) Cost {
	var total Cost
	for _, d := range decisions {
		total.Tokens += d.Cost.Tokens
		total.Duration += d.Cost.Duration
		total.TokensUSD += d.Cost.TokensUSD
		if d.Cost.Attempts > total.Attempts {
			total.Attempts = d.Cost.Attempts
		}
		total.RetryCount += d.Cost.RetryCount
	}
	return total
}

func matchesCategory(c Category, cats []Category) bool {
	for _, cat := range cats {
		if c == cat {
			return true
		}
	}
	return false
}

func matchesTerm(d DecisionReceipt, term string) bool {
	term = strings.ToLower(term)
	return strings.Contains(strings.ToLower(d.Decision), term) ||
		strings.Contains(strings.ToLower(d.Rationale), term)
}
