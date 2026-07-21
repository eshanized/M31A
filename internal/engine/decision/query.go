package decision

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
