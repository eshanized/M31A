package git

import (
	"fmt"
	"strconv"
	"strings"
	"time"
)

// BlameLine is a single blamed row from git blame --porcelain output.
// SHA identifies the commit that last touched the line, OrigLine is the
// line number in that commit's version of the file, FinalLine the number
// in the blamed revision, and Author/AuthorTime are resolved from the
// SHA-keyed metadata cache so suppressed repeat groups still carry them.
type BlameLine struct {
	SHA        string
	OrigLine   int
	FinalLine  int
	Author     string
	AuthorTime time.Time
}

// CommitMeta is the deduplicated per-commit metadata collected while
// parsing porcelain output. One entry exists per unique commit regardless
// of how many line groups it owns.
type CommitMeta struct {
	SHA         string
	Author      string
	AuthorEmail string
	Summary     string
	AuthorTime  time.Time
}

// ValidateRef reports whether ref is a safe git ref name. It delegates to
// the package's injection guard so out-of-package callers (intelligence
// commands) get the same protection as internal wrappers.
func ValidateRef(ref string) bool {
	return validateGitRef(ref)
}

// BlamePorcelain runs `git blame --porcelain` on path (repo-relative) at
// HEAD, or at rev when one is supplied, and returns one BlameLine per file
// row plus the deduplicated commit metadata ordered by first appearance.
//
// Porcelain prints a commit's tag lines (author, author-mail, author-time,
// summary, filename, boundary, ...) only on FIRST sight of its header;
// every later group from the same commit emits bare header + TAB content
// lines. The parser therefore maintains a cache keyed on the 40-hex header
// token and resolves Author/AuthorTime for suppressed groups from it
// (RESEARCH Pattern 2 / Pitfall 1). Unrecognized tag words between the
// header and the filename are skipped silently, per the official format.
func (g *Git) BlamePorcelain(path string, rev ...string) ([]BlameLine, []CommitMeta, error) {
	args := []string{"blame", "--porcelain"}
	if len(rev) > 0 && rev[0] != "" {
		if !validateGitRef(rev[0]) {
			return nil, nil, fmt.Errorf("git blame: invalid ref name %q", rev[0])
		}
		args = append(args, rev[0])
	}
	args = append(args, "--", path)

	out, err := g.run(args...)
	if err != nil {
		return nil, nil, fmt.Errorf("git blame --porcelain: %w", err)
	}

	blames, commits, perr := parseBlamePorcelain(out)
	if perr != nil {
		return nil, nil, fmt.Errorf("git blame parse: %w", perr)
	}
	return blames, commits, nil
}

// isHexSHA reports whether tok is a porcelain header SHA token: 40 hex
// characters, optionally caret-prefixed when the commit sits beyond a
// truncated history boundary.
func isHexSHA(tok string) bool {
	if strings.HasPrefix(tok, "^") {
		tok = tok[1:]
	}
	if len(tok) != 40 {
		return false
	}
	for i := 0; i < len(tok); i++ {
		c := tok[i]
		isDigit := c >= '0' && c <= '9'
		isHexLower := c >= 'a' && c <= 'f'
		if !isDigit && !isHexLower {
			return false
		}
	}
	return true
}

// isHeaderLine reports whether the line is a porcelain group-start or
// continuation header: "<sha> <orig-line> <final-line> [<num-lines>]".
// Continuation lines omit num-lines; both forms carry exactly 40-hex first.
func isHeaderLine(line string) bool {
	fields := strings.Fields(line)
	if len(fields) < 3 || len(fields) > 4 {
		return false
	}
	if !isHexSHA(fields[0]) {
		return false
	}
	for _, f := range fields[1:3] {
		if _, err := strconv.Atoi(f); err != nil {
			return false
		}
	}
	return true
}

// parseBlamePorcelain parses raw porcelain output statefully. It is
// unexported so tests can feed captured fixtures without shelling out to
// real git.
func parseBlamePorcelain(out string) ([]BlameLine, []CommitMeta, error) {
	var blames []BlameLine
	metaBySHA := make(map[string]*CommitMeta)
	var order []string // unique SHAs by first appearance

	// cur is the pending blame row whose TAB content line has not arrived
	// yet; meta is the cached CommitMeta backing it.
	var cur *BlameLine
	var meta *CommitMeta

	flush := func() {
		if cur == nil {
			return
		}
		cur.Author = meta.Author
		cur.AuthorTime = meta.AuthorTime
		blames = append(blames, *cur)
		cur = nil
	}

	for _, line := range strings.Split(out, "\n") {
		if strings.HasPrefix(line, "\t") {
			// Content row — completes the current group entry.
			flush()
			continue
		}
		line = strings.TrimRight(line, "\r")
		if line == "" {
			continue
		}

		if isHeaderLine(line) {
			// A new header always terminates any pending row defensively.
			flush()

			fields := strings.Fields(line)
			orig, _ := strconv.Atoi(fields[1])
			final, _ := strconv.Atoi(fields[2])
			cur = &BlameLine{SHA: fields[0], OrigLine: orig, FinalLine: final}

			existing, seen := metaBySHA[cur.SHA]
			if !seen {
				existing = &CommitMeta{SHA: cur.SHA}
				metaBySHA[cur.SHA] = existing
				order = append(order, cur.SHA)
			}
			meta = existing
			continue
		}

		// Tag line (or unknown word) — only meaningful while consuming the
		// first-seen metadata block for a commit. Tags before any header or
		// after a suppressed repeat header carry nothing to update.
		fields := strings.Fields(line)
		if len(fields) == 0 {
			continue
		}
		if meta == nil || (len(fields) < 2 && fields[0] != "boundary") {
			continue
		}
		switch fields[0] {
		case "author":
			meta.Author = strings.Join(fields[1:], " ")
		case "author-mail":
			meta.AuthorEmail = strings.TrimSuffix(strings.TrimPrefix(strings.Join(fields[1:], " "), "<"), ">")
		case "author-time":
			if sec, err := strconv.ParseInt(fields[1], 10, 64); err == nil {
				meta.AuthorTime = time.Unix(sec, 0).UTC()
			}
		case "summary":
			meta.Summary = strings.Join(fields[1:], " ")
		default:
			// author-tz, committer*, previous, filename, boundary, and any
			// future unknown tags are ignored silently per the spec.
		}
	}
	flush()

	commits := make([]CommitMeta, 0, len(order))
	for _, sha := range order {
		commits = append(commits, *metaBySHA[sha])
	}
	return blames, commits, nil
}
