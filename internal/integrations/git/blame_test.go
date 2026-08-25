package git

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

// blameFixture is a real `git blame --porcelain` capture from a two-hunk
// repository: commit da17163... (Jane Dev) created the file and still owns
// three line groups; commit 4e83b4f... (Bob Maint) rewrote two middle lines.
// Porcelain prints each commit's metadata ONCE — every later group from the
// same SHA emits bare header + TAB content lines (suppressed metadata,
// RESEARCH Pattern 2 / Pitfall 1).
const blameFixture = `da17163c2886a751570d955ee1de733115fec312 1 1 4
author Jane Dev
author-mail <jane@example.com>
author-time 1767607200
author-tz +0000
committer Jane Dev
committer-mail <jane@example.com>
committer-time 1767607200
committer-tz +0000
summary Initial bar implementation
boundary
filename bar.go
	package foo
da17163c2886a751570d955ee1de733115fec312 2 2

da17163c2886a751570d955ee1de733115fec312 3 3
	// Bar returns a value.
da17163c2886a751570d955ee1de733115fec312 4 4
	func Bar() int {
4e83b4fe244c3649525668c6931918739fe10cae 5 5 1
author Bob Maint
author-mail <bob@example.com>
author-time 1770726600
author-tz +0000
committer Bob Maint
committer-mail <bob@example.com>
committer-time 1770726600
committer-tz +0000
summary Update return values and document Baz
previous da17163c2886a751570d955ee1de733115fec312 bar.go
filename bar.go
		return 100
da17163c2886a751570d955ee1de733115fec312 6 6 2
	}
da17163c2886a751570d955ee1de733115fec312 7 7

4e83b4fe244c3649525668c6931918739fe10cae 8 8 1
	// Baz doubled for clarity.
da17163c2886a751570d955ee1de733115fec312 8 9 3
	func Baz() int {
da17163c2886a751570d955ee1de733115fec312 9 10
		return 2
da17163c2886a751570d955ee1de733115fec312 10 11
	}
`

func TestParseBlamePorcelain_FixtureTwoHunks(t *testing.T) {
	blames, commits, err := parseBlamePorcelain(blameFixture)
	if err != nil {
		t.Fatalf("parseBlamePorcelain failed: %v", err)
	}

	// 11 content rows expected.
	if len(blames) != 11 {
		t.Fatalf("expected 11 blame lines, got %d", len(blames))
	}

	janeSHA := "da17163c2886a751570d955ee1de733115fec312"
	bobSHA := "4e83b4fe244c3649525668c6931918739fe10cae"

	wantAuthors := map[int]string{
		1: "Jane Dev", 2: "Jane Dev", 3: "Jane Dev", 4: "Jane Dev",
		5: "Bob Maint",
		6: "Jane Dev", 7: "Jane Dev",
		8: "Bob Maint",
		9: "Jane Dev", 10: "Jane Dev", 11: "Jane Dev",
	}
	for _, bl := range blames {
		want, ok := wantAuthors[bl.FinalLine]
		if !ok {
			t.Fatalf("unexpected final line %d", bl.FinalLine)
		}
		if bl.Author != want {
			t.Errorf("final line %d: author = %q, want %q", bl.FinalLine, bl.Author, want)
		}
		if bl.FinalLine <= 4 || bl.FinalLine == 6 || bl.FinalLine == 7 || bl.FinalLine >= 9 {
			if bl.SHA != janeSHA {
				t.Errorf("final line %d: sha = %q, want Jane's %q", bl.FinalLine, bl.SHA, janeSHA)
			}
		} else if bl.SHA != bobSHA {
			t.Errorf("final line %d: sha = %q, want Bob's %q", bl.FinalLine, bl.SHA, bobSHA)
		}
	}

	// Suppressed-metadata core assertion: Jane owns final lines 1 AND 6 from
	// two DIFFERENT groups; the second group emitted no author tags, yet the
	// SHA-keyed cache must populate Author on both.
	if blames[0].Author != "Jane Dev" {
		t.Errorf("first group author = %q, want Jane Dev", blames[0].Author)
	}
	if blames[5].Author != "Jane Dev" {
		t.Errorf("second Jane group (final line 6) author = %q, want Jane Dev (metadata suppression broke attribution)", blames[5].Author)
	}

	// Continuation header WITHOUT num-lines must not misread line numbers:
	// "da17163... 2 2" means orig 2, final 2.
	if blames[1].OrigLine != 2 || blames[1].FinalLine != 2 {
		t.Errorf("continuation line parsed as orig=%d final=%d, want 2/2", blames[1].OrigLine, blames[1].FinalLine)
	}
	// Header WITH num-lines: "da17163... 8 9 3" means orig 8, final 9.
	if blames[8].OrigLine != 8 || blames[8].FinalLine != 9 {
		t.Errorf("num-lines header parsed as orig=%d final=%d, want 8/9", blames[8].OrigLine, blames[8].FinalLine)
	}

	// AuthorTime from the first Jane group: unix 1767607200 UTC.
	wantTime := time.Unix(1767607200, 0).UTC()
	if !blames[0].AuthorTime.Equal(wantTime) {
		t.Errorf("author time = %v, want %v", blames[0].AuthorTime, wantTime)
	}

	// Commit slice: deduplicated, ordered by first appearance.
	if len(commits) != 2 {
		t.Fatalf("expected 2 unique commits, got %d", len(commits))
	}
	if commits[0].SHA != janeSHA || commits[1].SHA != bobSHA {
		t.Errorf("commit order = [%s, %s], want [jane, bob]", commits[0].SHA, commits[1].SHA)
	}
	if commits[0].AuthorEmail != "jane@example.com" {
		t.Errorf("jane email = %q, want jane@example.com", commits[0].AuthorEmail)
	}
	if commits[0].Summary != "Initial bar implementation" {
		t.Errorf("jane summary = %q", commits[0].Summary)
	}
	if commits[1].Summary != "Update return values and document Baz" {
		t.Errorf("bob summary = %q", commits[1].Summary)
	}
	if !commits[1].AuthorTime.Equal(time.Unix(1770726600, 0).UTC()) {
		t.Errorf("bob author time = %v", commits[1].AuthorTime)
	}
}

func TestParseBlamePorcelain_ToleratesBoundaryAndUnknownTags(t *testing.T) {
	fixture := "aaaaaaaaaabbbbbbbbbbccccccccccdddddddddd 1 1 2\n" +
		"author Ann\n" +
		"invented-future-tag some value parsers never heard of\n" +
		"another-unknown 42\n" +
		"boundary\n" +
		"\tfirst\n" +
		"aaaaaaaaaabbbbbbbbbbccccccccccdddddddddd 2 2\n" +
		"\tsecond\n"

	blames, commits, err := parseBlamePorcelain(fixture)
	if err != nil {
		t.Fatalf("parseBlamePorcelain failed: %v", err)
	}
	if len(blames) != 2 {
		t.Fatalf("expected 2 blame lines, got %d", len(blames))
	}
	for _, bl := range blames {
		if bl.Author != "Ann" {
			t.Errorf("final line %d: author = %q, want Ann (unknown tags must be skipped silently)", bl.FinalLine, bl.Author)
		}
	}
	if len(commits) != 1 {
		t.Fatalf("expected 1 unique commit, got %d", len(commits))
	}
}

func TestValidateRef(t *testing.T) {
	cases := []struct {
		ref  string
		want bool
	}{
		{"main", true},
		{"HEAD", true},
		{"refs/heads/main", true},
		{"v1.2.3", true},
		{"", true}, // empty handled by caller (no rev)
		{"--upload-pack=/bin/evil", false},
		{"--exec=cmd", false},
		{"abc;rm -rf /", false},
		{"$(calc)", false},
		{"a..b", false},
		{"has space", false},
		{"back`tick", false},
		{".hidden", false},
	}
	for _, tc := range cases {
		if got := ValidateRef(tc.ref); got != tc.want {
			t.Errorf("ValidateRef(%q) = %v, want %v", tc.ref, got, tc.want)
		}
	}
}

func TestBlamePorcelain_HostileRevFailsFast(t *testing.T) {
	// Non-repo directory: if the rev ever reached exec git, the error would
	// be git's "fatal: not a git repository" instead of our validation error.
	g := New(t.TempDir())

	hostile := []string{
		"--upload-pack=/tmp/evil",
		"abc;touch /tmp/pwned",
		"main..secret",
	}
	for _, rev := range hostile {
		blames, commits, err := g.BlamePorcelain("bar.go", rev)
		if err == nil {
			t.Fatalf("BlamePorcelain with hostile rev %q: expected error", rev)
		}
		if !strings.Contains(err.Error(), "invalid ref") {
			t.Errorf("hostile rev %q: error = %v, want fast validation error mentioning invalid ref (git may have executed)", rev, err)
		}
		if strings.Contains(err.Error(), "fatal:") || strings.Contains(err.Error(), "not a git repository") {
			t.Errorf("hostile rev %q reached git execution before validation", rev)
		}
		if blames != nil || commits != nil {
			t.Errorf("hostile rev %q: expected nil results", rev)
		}
	}
}

func TestBlamePorcelain_RealRepoSuppressedMetadata(t *testing.T) {
	g, dir := setupRepo(t)

	// Commit 1 by Alice creates a file whose lines survive into two groups.
	v1 := "alpha\nbeta\ngamma\ndelta\nepsilon\n"
	if err := os.WriteFile(filepath.Join(dir, "sample.txt"), []byte(v1), 0644); err != nil {
		t.Fatalf("write v1: %v", err)
	}
	if err := g.Add("sample.txt"); err != nil {
		t.Fatalf("add: %v", err)
	}
	if _, err := g.CommitStaged("initial sample"); err != nil {
		t.Fatalf("commit 1: %v", err)
	}

	// Commit 2 by Bob rewrites the middle, splitting Alice's lines into two hunks.
	if err := g.ConfigUser("Bob Two", "bob2@test.com"); err != nil {
		t.Fatalf("ConfigUser bob: %v", err)
	}
	v2 := "alpha\nbeta\nGAMMA\ngamma\ndelta\nepsilon\n"
	if err := os.WriteFile(filepath.Join(dir, "sample.txt"), []byte(v2), 0644); err != nil {
		t.Fatalf("write v2: %v", err)
	}
	if err := g.Add("sample.txt"); err != nil {
		t.Fatalf("add v2: %v", err)
	}
	if _, err := g.CommitStaged("middle rewrite"); err != nil {
		t.Fatalf("commit 2: %v", err)
	}

	blames, commits, err := g.BlamePorcelain("sample.txt")
	if err != nil {
		t.Fatalf("BlamePorcelain failed: %v", err)
	}
	if len(blames) != 6 {
		t.Fatalf("expected 6 blame lines, got %d", len(blames))
	}

	aliceRows, bobRows := 0, 0
	for _, bl := range blames {
		switch bl.Author {
		case "Test User":
			aliceRows++
			if bl.AuthorTime.IsZero() {
				t.Errorf("final line %d: Alice author time zero despite suppressed metadata", bl.FinalLine)
			}
		case "Bob Two":
			bobRows++
		default:
			t.Errorf("final line %d: unexpected author %q", bl.FinalLine, bl.Author)
		}
	}
	// Alice owns alpha,beta and then gamma,delta,epsilon — two groups split
	// by Bob's inserted GAMMA line, so the second group relies on the
	// SHA-keyed cache for suppressed metadata. Bob owns only GAMMA.
	if aliceRows != 5 || bobRows != 1 {
		t.Errorf("author split alice=%d bob=%d, want 5/1 (suppressed metadata lost attribution?)", aliceRows, bobRows)
	}
	if len(commits) != 2 {
		t.Errorf("expected 2 commits, got %d", len(commits))
	}
}
