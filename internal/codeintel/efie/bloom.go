package efie

// BloomFilter provides O(1) probabilistic membership tests.
type BloomFilter struct {
	bits    []uint64
	numHash int
	size    uint
}

// NewBloomFilter creates a bloom filter for expectedItems with the given
// false positive rate. ~1.2 bytes per element at 1% FP rate.
func NewBloomFilter(expectedItems int, fpRate float64) *BloomFilter {
	n := float64(expectedItems)
	if n < 1 {
		n = 1
	}
	p := fpRate
	if p <= 0 || p >= 1 {
		p = 0.01
	}
	// Optimal size: m = -n * ln(p) / (ln2)^2
	m := -n * ln(p) / (log2 * log2)
	k := m / n * log2
	if k < 1 {
		k = 1
	}
	size := uint(m + 0.5)
	if size < 64 {
		size = 64
	}
	return &BloomFilter{
		bits:    make([]uint64, (size+63)/64),
		numHash: int(k),
		size:    size,
	}
}

const log2 = 0.6931471805599453

// ln computes natural logarithm using Taylor series
func ln(x float64) float64 {
	if x <= 0 {
		return -1e10
	}
	// Normalize: ln(x) = ln(x/e^k) + k
	k := 0.0
	for x > 2 {
		x /= e
		k++
	}
	for x < 0.5 {
		x *= e
		k--
	}
	// Taylor series for ln(1+t) where t = x-1
	t := x - 1
	result := t
	term := t
	for i := 2; i <= 30; i++ {
		term *= -t
		result += term / float64(i)
	}
	return k + result
}

const e = 2.718281828459045

// Add inserts an item into the bloom filter.
func (bf *BloomFilter) Add(item string) {
	for i := 0; i < bf.numHash; i++ {
		h := bf.hash(item, i) % bf.size
		bf.bits[h/64] |= 1 << (h % 64)
	}
}

// Contains returns true if the item is probably in the set,
// false if definitely not. O(1) time.
func (bf *BloomFilter) Contains(item string) bool {
	for i := 0; i < bf.numHash; i++ {
		h := bf.hash(item, i) % bf.size
		if bf.bits[h/64]&(1<<(h%64)) == 0 {
			return false
		}
	}
	return true
}

// hash computes a hash using FNV-1a-like mixing with seed.
func (bf *BloomFilter) hash(item string, seed int) uint {
	// Use two different FNV offset basis values mixed with seed
	h1 := uint64(14695981039346656037) // FNV-1a offset basis
	h2 := uint64(1099511628211)       // FNV offset basis
	seed64 := uint64(seed)

	for i := 0; i < len(item); i++ {
		c := uint64(item[i])
		h1 ^= c
		h1 *= 1099511628211
		h2 ^= c
		h2 *= 14695981039346656037
	}
	h1 ^= seed64
	h1 *= 1099511628211
	h2 ^= seed64 << 32
	h2 *= 14695981039346656037

	return uint((h1 ^ h2) >> 16)
}
