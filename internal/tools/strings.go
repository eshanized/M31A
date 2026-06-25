package tools

// LevenshteinDistance computes the edit distance between two strings.
func LevenshteinDistance(a, b string) int {
	la, lb := len(a), len(b)
	if la == 0 {
		return lb
	}
	if lb == 0 {
		return la
	}
	prev := make([]int, lb+1)
	curr := make([]int, lb+1)
	for j := 0; j <= lb; j++ {
		prev[j] = j
	}
	for i := 1; i <= la; i++ {
		curr[0] = i
		for j := 1; j <= lb; j++ {
			cost := 1
			if a[i-1] == b[j-1] {
				cost = 0
			}
			curr[j] = min(curr[j-1]+1, min(prev[j]+1, prev[j-1]+cost))
		}
		prev, curr = curr, prev
	}
	return prev[lb]
}

// LevenshteinBuf is like LevenshteinDistance but reuses pre-allocated buffers.
func LevenshteinBuf(a, b string, prev, curr []int) int {
	la, lb := len(a), len(b)
	if la == 0 {
		return lb
	}
	if lb == 0 {
		return la
	}
	for j := 0; j <= lb; j++ {
		prev[j] = j
	}
	for i := 1; i <= la; i++ {
		curr[0] = i
		for j := 1; j <= lb; j++ {
			cost := 1
			if a[i-1] == b[j-1] {
				cost = 0
			}
			curr[j] = min(curr[j-1]+1, min(prev[j]+1, prev[j-1]+cost))
		}
		prev, curr = curr, prev
	}
	return prev[lb]
}

// HumanSize formats bytes into a human-readable string.
func HumanSize(b int64) string {
	const unit = 1024
	if b < unit {
		return formatBytes(b)
	}
	div, exp := int64(unit), 0
	for n := b / unit; n >= unit; n /= unit {
		div *= unit
		exp++
	}
	units := "KMGTPE"
	if exp >= len(units) {
		exp = len(units) - 1
		div = int64(unit)
		for i := 1; i <= exp; i++ {
			div *= unit
		}
	}
	return formatBytesWithUnit(float64(b)/float64(div), units[exp])
}

func formatBytes(b int64) string {
	return formatInt(b) + " B"
}

func formatBytesWithUnit(val float64, unit byte) string {
	return formatFloat(val) + " " + string(unit) + "B"
}

func formatInt(n int64) string {
	if n == 0 {
		return "0"
	}
	buf := [20]byte{}
	i := len(buf)
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	return string(buf[i:])
}

func formatFloat(f float64) string {
	// Simple formatting: one decimal place
	intPart := int64(f)
	decPart := int64((f - float64(intPart)) * 10)
	if decPart < 0 {
		decPart = -decPart
	}
	return formatInt(intPart) + "." + formatInt(decPart)
}
