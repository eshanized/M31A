// Package ledger implements a cross-session learning ledger that records
// every shipped session's metadata to a persistent markdown file
// (~/.m31a/LEDGER.md). The ledger supports append, filtered queries,
// aggregate statistics, and automatic truncation.
package ledger
