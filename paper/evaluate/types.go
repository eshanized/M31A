package main

// ScoredFile mirrors efie.ScoredFile for benchmarking.
type ScoredFile struct {
	Path    string
	Score   float64
	Reasons []string
}

// ScalePoint holds a single data point for scalability analysis.
type ScalePoint struct {
	FileCount   int
	EFIEBuildMs float64
	OrigBuildMs float64
	EFIEQueryMs float64
	OrigQueryMs float64
}
