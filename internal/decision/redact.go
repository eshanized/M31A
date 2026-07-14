package decision

import "regexp"

var (
	apiKeyPattern = regexp.MustCompile(`(?i)(api[_-]?key|token|secret|password|credential|bearer)\s*[:=]\s*\S+`)
	bearerPattern = regexp.MustCompile(`(?i)bearer\s+[A-Za-z0-9._\-]+`)
	emailPattern  = regexp.MustCompile(`[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}`)
	ipPattern     = regexp.MustCompile(`\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b`)
)

// RedactReceipt returns a copy with sensitive data masked.
func RedactReceipt(r DecisionReceipt) DecisionReceipt {
	r.Decision = RedactString(r.Decision)
	r.Rationale = RedactString(r.Rationale)
	orig := r.Alternatives
	if orig != nil {
		r.Alternatives = make([]string, len(orig))
		for i, alt := range orig {
			r.Alternatives[i] = RedactString(alt)
		}
	}
	return r
}

// RedactString masks sensitive patterns in a string.
func RedactString(s string) string {
	s = apiKeyPattern.ReplaceAllString(s, "$1=***REDACTED***")
	s = bearerPattern.ReplaceAllString(s, "Bearer ***REDACTED***")
	s = emailPattern.ReplaceAllString(s, "***@***.***")
	s = ipPattern.ReplaceAllString(s, "***.***.***.***")
	return s
}

// RedactSlice redacts a slice of receipts.
func RedactSlice(decisions []DecisionReceipt) []DecisionReceipt {
	result := make([]DecisionReceipt, len(decisions))
	for i, d := range decisions {
		result[i] = RedactReceipt(d)
	}
	return result
}
