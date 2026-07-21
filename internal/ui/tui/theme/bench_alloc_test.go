package theme

import "testing"

func BenchmarkBuildSemanticStyles(b *testing.B) {
	t := M31A()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		_ = BuildSemanticStyles(t)
	}
}

func BenchmarkStyleCache_S(b *testing.B) {
	t := M31A()
	cache := NewStyleCache(t)
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		_ = cache.S
	}
}
