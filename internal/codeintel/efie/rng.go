package efie

import "math/rand"

// simpleRng wraps math/rand for deterministic operations.
type simpleRng struct {
	rng *rand.Rand
}

func newSeededRng(seed int64) *simpleRng {
	return &simpleRng{rng: rand.New(rand.NewSource(seed))}
}

func (r *simpleRng) Intn(n int) int {
	return r.rng.Intn(n)
}

func (r *simpleRng) Shuffle(n int, swap func(i, j int)) {
	r.rng.Shuffle(n, swap)
}

func (r *simpleRng) Perm(n int) []int {
	return r.rng.Perm(n)
}
