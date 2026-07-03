package logging

import (
	"bufio"
	"os"
	"regexp"
	"strings"
)

// secretPatterns matches variable names or log arguments that may contain secrets.
var secretPatterns = []*regexp.Regexp{
	regexp.MustCompile(`(?i)(api[_-]?key|secret|token|password|credential|auth[_-]?token)`),
	regexp.MustCompile(`(?i)(bearer\s+[A-Za-z0-9\-._~+/]+=*)`),
	regexp.MustCompile(`(?i)(sk-[A-Za-z0-9]{20,})`),
}

// RedactSecrets replaces common secret patterns in a string with a redacted placeholder.
// It masks API keys, bearer tokens, and other sensitive values while preserving
// the surrounding text for debugging.
func RedactSecrets(s string) string {
	// Mask sk-* style API keys (e.g., OpenAI)
	s = regexp.MustCompile(`sk-[A-Za-z0-9\-]{20,}`).ReplaceAllString(s, "sk-****")

	// Mask bearer tokens
	s = regexp.MustCompile(`(?i)(bearer\s+)[A-Za-z0-9\-._~+/]{10,}=?=?`).ReplaceAllString(s, "${1}****")

	// Mask long hex strings that look like tokens (32+ chars)
	s = regexp.MustCompile(`\b[A-Fa-f0-9]{32,}\b`).ReplaceAllString(s, "****")

	return s
}

// SanitizeLogValue returns a redacted value for sensitive keys, or the original value
// for non-sensitive keys. Intended for use with slog group attributes.
func SanitizeLogValue(key string, value any) any {
	lower := strings.ToLower(key)
	if strings.Contains(lower, "key") || strings.Contains(lower, "secret") ||
		strings.Contains(lower, "token") || strings.Contains(lower, "password") ||
		strings.Contains(lower, "credential") || strings.Contains(lower, "auth") {
		return "****"
	}
	if s, ok := value.(string); ok {
		return RedactSecrets(s)
	}
	return value
}

// AuditLogForSecrets scans a Go source file for log statements that might
// leak secrets. It returns a list of human-readable issue descriptions.
// This is a static analysis tool — it looks for patterns like logging
// variables named "key", "secret", "token", "password" without redaction.
func AuditLogForSecrets(filename string) ([]string, error) {
	f, err := os.Open(filename)
	if err != nil {
		return nil, err
	}
	defer func() { _ = f.Close() }()

	var issues []string
	lineNum := 0
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		lineNum++
		line := scanner.Text()
		trimmed := strings.TrimSpace(line)

		// Skip comments and non-log lines
		if strings.HasPrefix(trimmed, "//") || !isLogLine(trimmed) {
			continue
		}

		// Check if a sensitive variable is logged without redaction
		for _, pat := range secretPatterns {
			matches := pat.FindAllString(line, -1)
			for _, m := range matches {
				// Skip if the line already redacts the value
				if strings.Contains(line, "redact") || strings.Contains(line, "sanitize") ||
					strings.Contains(line, "mask") || strings.Contains(line, "****") {
					continue
				}
				issues = append(issues, strings.TrimSpace(
					filename+":"+itoa(lineNum)+": possible secret in log: "+m))
			}
		}
	}
	return issues, scanner.Err()
}

// isLogLine returns true if the line appears to be a logging statement.
func isLogLine(line string) bool {
	return strings.Contains(line, "slog.") ||
		strings.Contains(line, "log.") ||
		strings.Contains(line, "fmt.Print") ||
		strings.Contains(line, "fmt.Fprintf") ||
		strings.Contains(line, "fmt.Sprint")
}

// itoa converts an int to its decimal string representation.
func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	var buf [20]byte
	i := len(buf)
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	return string(buf[i:])
}
