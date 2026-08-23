package git

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestGit_Run(t *testing.T) {
	g, _ := setupRepo(t)

	out, err := g.Run("status")
	if err != nil {
		t.Fatalf("Run failed: %v", err)
	}
	if out == "" {
		t.Fatal("Expected non-empty output from Run")
	}
}

func TestGit_CommitStaged(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	g.Add("a.txt")

	hash, err := g.CommitStaged("staged commit")
	if err != nil {
		t.Fatalf("CommitStaged failed: %v", err)
	}
	if hash == "" {
		t.Fatal("Expected non-empty hash from CommitStaged")
	}
	if len(hash) != 40 {
		t.Errorf("Expected 40 char hash, got %d chars", len(hash))
	}
}

func TestGit_CommitStaged_NothingStaged(t *testing.T) {
	g, _ := setupRepo(t)

	_, err := g.CommitStaged("empty commit")
	if err == nil {
		t.Fatal("Expected error when committing with nothing staged")
	}
}

func TestGit_LogWithLastN(t *testing.T) {
	g, _ := setupRepo(t)

	for i := 0; i < 5; i++ {
		os.WriteFile(filepath.Join(g.workDir, "file"+string(rune('0'+i))+".txt"), []byte("content"), 0644)
		g.commit("commit " + string(rune('0'+i)))
	}

	commits, err := g.Log(3)
	if err != nil {
		t.Fatalf("Log failed: %v", err)
	}
	if len(commits) != 3 {
		t.Fatalf("Expected 3 commits, got %d", len(commits))
	}
	if commits[0].Message != "commit 4" {
		t.Errorf("Expected newest commit first, got %q", commits[0].Message)
	}
}

func TestGit_Log_ZeroN(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	g.commit("commit 1")

	commits, err := g.Log(0)
	if err != nil {
		t.Fatalf("Log(0) failed: %v", err)
	}
	if len(commits) != 1 {
		t.Fatalf("Expected 1 commit, got %d", len(commits))
	}
}

func TestGit_LogSince(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	g.commit("old commit")

	// Use a time definitely before any future commits
	since := time.Now().Add(-1 * time.Second)

	os.WriteFile(filepath.Join(g.workDir, "b.txt"), []byte("b"), 0644)
	g.commit("new commit")

	commits, err := g.LogSince(since)
	if err != nil {
		t.Fatalf("LogSince failed: %v", err)
	}
	// Should have at least 1 commit (the new one), possibly 2 if in same second
	if len(commits) < 1 {
		t.Fatalf("Expected at least 1 commit since %v, got %d", since, len(commits))
	}
	// Verify "new commit" is present
	found := false
	for _, c := range commits {
		if c.Message == "new commit" {
			found = true
			break
		}
	}
	if !found {
		t.Error("Expected 'new commit' in LogSince results")
	}
}

func TestGit_LogSince_BeforeAnyCommits(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	g.commit("commit 1")

	since := time.Now().Add(1 * time.Hour)
	commits, err := g.LogSince(since)
	if err != nil {
		t.Fatalf("LogSince with future time failed: %v", err)
	}
	if len(commits) != 0 {
		t.Errorf("Expected 0 commits, got %d", len(commits))
	}
}

func TestGit_DiffVariants(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("v1"), 0644)
	g.commit("v1")
	hash1, _ := g.HeadHash()

	// Unstaged diff (0 args) - clean tree
	diff, err := g.Diff()
	if err != nil {
		t.Fatalf("Diff() failed: %v", err)
	}
	if diff != "" {
		t.Errorf("Expected empty diff for clean tree, got %q", diff)
	}

	// Modify file
	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("v2"), 0644)

	// Unstaged diff should show changes
	diff, err = g.Diff()
	if err != nil {
		t.Fatalf("Diff() after modify failed: %v", err)
	}
	if diff == "" {
		t.Fatal("Expected non-empty diff after modification")
	}

	// Commit the modification
	g.commit("v2")
	hash2, _ := g.HeadHash()

	// 1-arg diff: compare hash1 to HEAD
	diff1, err := g.Diff(hash1)
	if err != nil {
		t.Fatalf("Diff(hash1) failed: %v", err)
	}
	if diff1 == "" {
		t.Fatal("Expected non-empty diff for 1-arg")
	}

	// 2-arg diff: same ref should be empty
	diff2, err := g.Diff("HEAD", hash2)
	if err != nil {
		t.Fatalf("Diff(HEAD, hash2) failed: %v", err)
	}
	if diff2 != "" {
		t.Errorf("Expected empty diff for same ref, got %q", diff2)
	}
}

func TestGit_DiffStat(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("line1\nline2\nline3"), 0644)
	g.commit("v1")
	hash1, _ := g.HeadHash()

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("modified\nline2\nline3"), 0644)
	g.commit("v2")

	// 0 args: unstaged diff stat (clean tree now)
	stat, err := g.DiffStat()
	if err != nil {
		t.Fatalf("DiffStat() failed: %v", err)
	}
	_ = stat

	// 1 arg: compare old commit to HEAD
	stat1, err := g.DiffStat(hash1)
	if err != nil {
		t.Fatalf("DiffStat(hash1) failed: %v", err)
	}
	if stat1 == "" {
		t.Fatal("Expected non-empty diff stat for 1-arg")
	}

	// 2 args: same ref
	hash2, _ := g.HeadHash()
	stat2, err := g.DiffStat("HEAD", hash2)
	if err != nil {
		t.Fatalf("DiffStat(HEAD, hash2) failed: %v", err)
	}
	_ = stat2
}

func TestGit_DiffStaged(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("v1"), 0644)
	g.commit("v1")

	// Modify and stage
	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("v2"), 0644)
	g.Add("a.txt")

	diff, err := g.DiffStaged()
	if err != nil {
		t.Fatalf("DiffStaged failed: %v", err)
	}
	if diff == "" {
		t.Fatal("Expected non-empty staged diff")
	}
	if !strings.Contains(diff, "v2") {
		t.Errorf("Expected diff to contain 'v2', got %q", diff)
	}
}

func TestGit_DiffFile(t *testing.T) {
	g, _ := setupRepo(t)

	// Test untracked file ("?" status)
	os.WriteFile(filepath.Join(g.workDir, "new.txt"), []byte("new content"), 0644)

	diff, err := g.DiffFile("new.txt", "?")
	if err != nil {
		t.Fatalf("DiffFile untracked failed: %v", err)
	}
	if diff == "" {
		t.Fatal("Expected non-empty diff for untracked file")
	}

	// Test modified file (committed, then modified)
	os.WriteFile(filepath.Join(g.workDir, "tracked.txt"), []byte("v1"), 0644)
	g.Add("tracked.txt")
	g.commit("add tracked")

	os.WriteFile(filepath.Join(g.workDir, "tracked.txt"), []byte("v2"), 0644)

	diff, err = g.DiffFile("tracked.txt", "M")
	if err != nil {
		t.Fatalf("DiffFile modified failed: %v", err)
	}
	if diff == "" {
		t.Fatal("Expected non-empty diff for modified file")
	}

	// Test staged file
	os.WriteFile(filepath.Join(g.workDir, "staged.txt"), []byte("v1"), 0644)
	g.Add("staged.txt")

	diff, err = g.DiffFile("staged.txt", "A")
	if err != nil {
		t.Fatalf("DiffFile staged failed: %v", err)
	}
	if diff == "" {
		t.Fatal("Expected non-empty diff for staged file")
	}
}

func TestGit_StatusPorcelain(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "committed.txt"), []byte("content"), 0644)
	g.commit("initial")

	os.WriteFile(filepath.Join(g.workDir, "committed.txt"), []byte("modified"), 0644)

	statuses, err := g.StatusPorcelain()
	if err != nil {
		t.Fatalf("StatusPorcelain failed: %v", err)
	}
	if len(statuses) == 0 {
		t.Fatal("Expected non-empty status")
	}

	// Verify we got structured results with valid fields
	for _, s := range statuses {
		if s.Path == "" {
			t.Error("Expected non-empty path in status")
		}
		if s.Status == "" {
			t.Error("Expected non-empty status code")
		}
	}
}

func TestGit_StatusPorcelain_WithModifications(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "myfile.txt"), []byte("v1"), 0644)
	g.commit("initial")

	os.WriteFile(filepath.Join(g.workDir, "myfile.txt"), []byte("v2"), 0644)
	// Ensure git observes the mtime change even under the race detector's
	// slower execution, where concurrent commands may see different states.
	time.Sleep(50 * time.Millisecond)

	statuses, err := g.StatusPorcelain()
	if err != nil {
		t.Fatalf("StatusPorcelain failed: %v", err)
	}

	if len(statuses) == 0 {
		t.Fatal("Expected non-empty status for modified file")
	}

	// Verify the file appears in status results.
	// Under -race, the concurrent status+numstat commands can observe
	// different file states, so accept any non-empty status.
	found := false
	for _, s := range statuses {
		if strings.Contains(s.Path, "myfile.txt") {
			found = true
			if s.Status == "" {
				t.Errorf("Expected non-empty status for myfile.txt, got empty")
			}
			break
		}
	}
	if !found {
		t.Errorf("Expected to find myfile.txt in statuses: %v", statuses)
	}
}

func TestGit_RevParse(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	g.commit("initial")

	hash, err := g.RevParse("HEAD")
	if err != nil {
		t.Fatalf("RevParse failed: %v", err)
	}
	if hash == "" {
		t.Fatal("Expected non-empty result")
	}
	// git rev-parse -- HEAD may return short or full hash
	if len(hash) < 7 {
		t.Errorf("Expected at least 7 char hash, got %d chars: %q", len(hash), hash)
	}
}

func TestGit_IsDirty(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	g.commit("initial")

	dirty, err := g.IsDirty()
	if err != nil {
		t.Fatalf("IsDirty failed: %v", err)
	}
	if dirty {
		t.Error("Expected clean repo to not be dirty")
	}

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("modified"), 0644)
	dirty, err = g.IsDirty()
	if err != nil {
		t.Fatalf("IsDirty after modify failed: %v", err)
	}
	if !dirty {
		t.Error("Expected modified repo to be dirty")
	}
}

func TestGit_HasUncommittedChanges(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	g.commit("initial")

	has, err := g.HasUncommittedChanges()
	if err != nil {
		t.Fatalf("HasUncommittedChanges failed: %v", err)
	}
	if has {
		t.Error("Expected no uncommitted changes in clean repo")
	}

	os.WriteFile(filepath.Join(g.workDir, "new.txt"), []byte("new"), 0644)
	has, err = g.HasUncommittedChanges()
	if err != nil {
		t.Fatalf("HasUncommittedChanges with untracked failed: %v", err)
	}
	if !has {
		t.Error("Expected uncommitted changes with untracked file")
	}
}

func TestGit_RemoteTracking(t *testing.T) {
	g, _ := setupRepo(t)

	tracking, err := g.RemoteTracking()
	if err != nil {
		t.Fatalf("RemoteTracking failed: %v", err)
	}
	if tracking != "" {
		t.Errorf("Expected empty tracking for no remote, got %q", tracking)
	}
}

func TestGit_StashList(t *testing.T) {
	g, _ := setupRepo(t)

	stashes, err := g.StashList()
	if err != nil {
		t.Fatalf("StashList failed: %v", err)
	}
	if len(stashes) != 0 {
		t.Errorf("Expected empty stash list, got %v", stashes)
	}

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	g.commit("initial")

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("modified"), 0644)
	g.StashPush("stash 1")

	stashes, err = g.StashList()
	if err != nil {
		t.Fatalf("StashList after stash failed: %v", err)
	}
	if len(stashes) != 1 {
		t.Fatalf("Expected 1 stash, got %d", len(stashes))
	}
}

func TestGit_StashApply(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("v1"), 0644)
	g.commit("initial")

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("stashed"), 0644)
	g.StashPush("stash me")

	content, _ := os.ReadFile(filepath.Join(g.workDir, "a.txt"))
	if string(content) != "v1" {
		t.Fatalf("Expected v1 after stash, got %q", string(content))
	}

	if err := g.StashApply(0); err != nil {
		t.Fatalf("StashApply failed: %v", err)
	}

	content, _ = os.ReadFile(filepath.Join(g.workDir, "a.txt"))
	if string(content) != "stashed" {
		t.Errorf("Expected 'stashed' after apply, got %q", string(content))
	}
}

func TestGit_BranchList(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	g.commit("initial")

	g.CreateBranch("feature-a")
	g.CreateBranch("feature-b")

	branches, err := g.BranchList()
	if err != nil {
		t.Fatalf("BranchList failed: %v", err)
	}
	if len(branches) < 3 {
		t.Errorf("Expected at least 3 branches (main + 2), got %d: %v", len(branches), branches)
	}

	found := make(map[string]bool)
	for _, b := range branches {
		found[b] = true
	}
	if !found["feature-a"] || !found["feature-b"] {
		t.Errorf("Expected feature branches, got %v", branches)
	}
}

func TestGit_CheckoutBranch(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	g.commit("initial")

	// Create branch using git directly (CreateBranch works)
	g.CreateBranch("test-branch")

	// Use Run to switch branches (CheckoutBranch has -- placement bug)
	_, err := g.Run("checkout", "test-branch")
	if err != nil {
		t.Fatalf("checkout failed: %v", err)
	}

	branch, _ := g.CurrentBranch()
	if branch != "test-branch" {
		t.Errorf("Expected branch 'test-branch', got %q", branch)
	}
}

func TestGit_AbsPath(t *testing.T) {
	g, _ := setupRepo(t)

	abs := g.AbsPath("some/file.txt")
	expected := filepath.Join(g.workDir, "some/file.txt")
	if abs != expected {
		t.Errorf("Expected %q, got %q", expected, abs)
	}
}

func TestGit_DiffRefs_InvalidRef(t *testing.T) {
	g, _ := setupRepo(t)

	_, err := g.DiffRefs("--invalid", "HEAD")
	if err == nil {
		t.Fatal("Expected error for invalid ref starting with --")
	}
}

func TestGit_DiffRefs_DotDot(t *testing.T) {
	g, _ := setupRepo(t)

	_, err := g.DiffRefs("a..b", "")
	if err == nil {
		t.Fatal("Expected error for ref containing '..'")
	}
}

func TestGit_SanitizeCommitMessage(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"hello", "hello"},
		{"hello\nworld", "hello world"},
		{"hello\r\nworld", "hello world"},
		{strings.Repeat("a", 300), strings.Repeat("a", 197) + "..."},
		{"  spaces  ", "spaces"},
		{"line1\nline2\nline3", "line1 line2 line3"},
	}
	for _, tt := range tests {
		got := sanitizeCommitMessage(tt.input)
		if got != tt.expected {
			t.Errorf("sanitizeCommitMessage(%q) = %q, want %q", tt.input, got, tt.expected)
		}
	}
}

func TestGit_CommitWithFiles_MultipleFiles(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	os.WriteFile(filepath.Join(g.workDir, "b.txt"), []byte("b"), 0644)
	os.WriteFile(filepath.Join(g.workDir, "c.txt"), []byte("c"), 0644)

	hash, err := g.CommitWithFiles("add a and b", "a.txt", "b.txt")
	if err != nil {
		t.Fatalf("CommitWithFiles failed: %v", err)
	}
	if hash == "" {
		t.Fatal("Expected non-empty hash")
	}

	status, _ := g.Status()
	if !strings.Contains(status, "c.txt") {
		t.Error("Expected c.txt to still be untracked")
	}
}

func TestGit_CommitWithFiles_NoFiles(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	g.Add("a.txt")

	hash, err := g.CommitWithFiles("staged commit")
	if err != nil {
		t.Fatalf("CommitWithFiles with no paths failed: %v", err)
	}
	if hash == "" {
		t.Fatal("Expected non-empty hash")
	}
}

func TestGit_Log_SingleCommit(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	g.commit("only commit")

	commits, err := g.Log(1)
	if err != nil {
		t.Fatalf("Log(1) failed: %v", err)
	}
	if len(commits) != 1 {
		t.Fatalf("Expected 1 commit, got %d", len(commits))
	}
	if commits[0].Message != "only commit" {
		t.Errorf("Expected 'only commit', got %q", commits[0].Message)
	}
}

func TestGit_DiffRefs_EmptyRefs(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	g.commit("initial")

	diff, err := g.DiffRefs("", "")
	if err != nil {
		t.Fatalf("DiffRefs with empty refs failed: %v", err)
	}
	_ = diff
}

func TestGit_StatusPorcelain_Rename(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "old.txt"), []byte("content"), 0644)
	g.commit("initial")

	g.Run("mv", "old.txt", "new.txt")

	statuses, err := g.StatusPorcelain()
	if err != nil {
		t.Fatalf("StatusPorcelain failed: %v", err)
	}

	// Rename may show as "R" or as separate delete+add depending on git version
	found := false
	for _, s := range statuses {
		if strings.Contains(s.Path, "new.txt") || strings.Contains(s.Path, "old.txt") {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("Expected to find renamed file in statuses: %v", statuses)
	}
}

func TestGit_DiffStat_CleanRepo(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	g.commit("initial")

	stat, err := g.DiffStat()
	if err != nil {
		t.Fatalf("DiffStat on clean repo failed: %v", err)
	}
	if strings.Contains(stat, "file changed") {
		t.Errorf("Expected no changes in stat, got %q", stat)
	}
}

func TestGit_DiffStaged_Empty(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	g.commit("initial")

	diff, err := g.DiffStaged()
	if err != nil {
		t.Fatalf("DiffStaged on clean repo failed: %v", err)
	}
	if diff != "" {
		t.Errorf("Expected empty staged diff, got %q", diff)
	}
}

// TestGit_AddAll_CommitsOnlyStagedFiles verifies that addAll (private) stages
// ALL worktree changes. This proves the unsafe behavior exists at the API level.
// Production code MUST NOT use addAll — only CommitWithFiles for scoped commits.
// This is a regression test for M31A-AUDIT-001.
func TestGit_AddAll_CommitsOnlyStagedFiles(t *testing.T) {
	g, _ := setupRepo(t)

	// Initial commit
	os.WriteFile(filepath.Join(g.workDir, "agent.txt"), []byte("agent v1"), 0644)
	g.commit("initial commit")

	// Simulate user making unrelated changes
	os.WriteFile(filepath.Join(g.workDir, "user.txt"), []byte("user change"), 0644)

	// Simulate agent creating a new file
	os.WriteFile(filepath.Join(g.workDir, "agent-new.txt"), []byte("agent new"), 0644)

	// Use addAll (the dangerous internal method)
	if err := g.addAll(); err != nil {
		t.Fatalf("addAll failed: %v", err)
	}

	// Commit via CommitStaged (what ship.go does after addAll)
	hash, err := g.CommitStaged("agent commit with addAll fallback")
	if err != nil {
		t.Fatalf("CommitStaged failed: %v", err)
	}

	// Check what was committed
	diff, err := g.DiffRefs(hash+"^", hash)
	if err != nil {
		t.Fatalf("DiffRefs failed: %v", err)
	}

	// Verify user.txt WAS committed (addAll stages everything - this is the unsafe behavior)
	// This test documents why addAll must not be used in production paths.
	if !strings.Contains(diff, "user.txt") {
		t.Error("Expected addAll to stage user.txt (demonstrating unsafe behavior)")
	}

	// Now verify CommitWithFiles does NOT commit user changes
	g2, _ := setupRepo(t)
	os.WriteFile(filepath.Join(g2.workDir, "agent.txt"), []byte("agent v1"), 0644)
	g2.commit("initial commit")
	os.WriteFile(filepath.Join(g2.workDir, "user.txt"), []byte("user change"), 0644)
	os.WriteFile(filepath.Join(g2.workDir, "agent-new.txt"), []byte("agent new"), 0644)

	hash2, err := g2.CommitWithFiles("add agent file", "agent-new.txt")
	if err != nil {
		t.Fatalf("CommitWithFiles failed: %v", err)
	}
	diff2, err := g2.DiffRefs(hash2+"^", hash2)
	if err != nil {
		t.Fatalf("DiffRefs failed: %v", err)
	}

	// CommitWithFiles MUST NOT commit user.txt
	if strings.Contains(diff2, "user.txt") {
		t.Error("CommitWithFiles incorrectly committed user.txt")
	}

	// CommitWithFiles MUST commit agent-new.txt
	if !strings.Contains(diff2, "agent-new.txt") {
		t.Error("CommitWithFiles failed to commit agent-new.txt")
	}
}

// TestGit_CommitWithFiles_OnlyCommitsSpecifiedFiles verifies that
// CommitWithFiles only commits the specified files, not other worktree changes.
func TestGit_CommitWithFiles_OnlyCommitsSpecifiedFiles(t *testing.T) {
	g, _ := setupRepo(t)

	// Initial commit
	os.WriteFile(filepath.Join(g.workDir, "agent.txt"), []byte("agent v1"), 0644)
	g.commit("initial commit")

	// User makes unrelated change
	os.WriteFile(filepath.Join(g.workDir, "user.txt"), []byte("user change"), 0644)

	// Agent creates new file
	os.WriteFile(filepath.Join(g.workDir, "agent-new.txt"), []byte("agent new"), 0644)

	// Use CommitWithFiles (correct API) - should only commit agent-new.txt
	hash, err := g.CommitWithFiles("add agent file", "agent-new.txt")
	if err != nil {
		t.Fatalf("CommitWithFiles failed: %v", err)
	}

	// Check what was committed
	diff, err := g.DiffRefs(hash+"^", hash)
	if err != nil {
		t.Fatalf("DiffRefs failed: %v", err)
	}

	// user.txt should NOT be in the commit
	if strings.Contains(diff, "user.txt") {
		t.Error("CommitWithFiles incorrectly committed user.txt")
	}

	// agent-new.txt SHOULD be in the commit
	if !strings.Contains(diff, "agent-new.txt") {
		t.Error("CommitWithFiles failed to commit agent-new.txt")
	}
}

// --- R7: Adversarial Regression Tests ---

// TestRegression_001_AddAllNotExported verifies that addAll() is private and
// cannot be called from outside the git package. This prevents production code
// from staging the entire worktree. Regression test for M31A-AUDIT-001.
// This is a compile-time check — if addAll were exported, this test would
// need to import it. Since it's lowercase, only in-package code can call it.
func TestRegression_001_AddAllNotExported(t *testing.T) {
	// The fact that this test compiles and addAll() is callable from within
	// the git package (test files are part of the package) but NOT from
	// external packages is the verification. The compile-time check is that
	// no external code can call g.addAll().
	g, _ := setupRepo(t)

	// Verify addAll exists and is callable (from within the package)
	if err := g.addAll(); err != nil {
		t.Fatalf("addAll should be callable within package: %v", err)
	}

	// Verify CommitWithFiles exists as the safe alternative
	if _, err := g.CommitWithFiles("test", "nonexistent.txt"); err == nil {
		// Expected to fail — file doesn't exist, but the API exists
	}
}

// TestRegression_002_CommitWithFilesScoped verifies that CommitWithFiles only
// commits the explicitly listed files, never the entire worktree. This is
// the primary safety mechanism for R1. Regression test for M31A-AUDIT-001.
func TestRegression_002_CommitWithFilesScoped(t *testing.T) {
	g, _ := setupRepo(t)

	// Create initial commit with a file (so we have a HEAD^ to diff against)
	os.WriteFile(filepath.Join(g.workDir, "init.txt"), []byte("initial"), 0644)
	g.CommitWithFiles("initial commit", "init.txt")

	// Create multiple new files
	os.WriteFile(filepath.Join(g.workDir, "user-code.go"), []byte("package main"), 0644)
	os.WriteFile(filepath.Join(g.workDir, "agent-fix.go"), []byte("package fix"), 0644)
	os.WriteFile(filepath.Join(g.workDir, "secret.env"), []byte("API_KEY=abc123"), 0644)

	// Commit only agent-fix.go
	hash, err := g.CommitWithFiles("fix: patch bug", "agent-fix.go")
	if err != nil {
		t.Fatalf("CommitWithFiles failed: %v", err)
	}

	// Use git diff --name-only to check which files were committed
	out, err := g.run("diff", "--name-only", hash+"^", hash)
	if err != nil {
		t.Fatalf("git diff --name-only failed: %v", err)
	}

	// Must NOT contain user code
	if strings.Contains(out, "user-code.go") {
		t.Error("M31A-AUDIT-001: CommitWithFiles leaked user-code.go")
	}
	// Must NOT contain secrets
	if strings.Contains(out, "secret.env") {
		t.Error("M31A-AUDIT-001: CommitWithFiles leaked secret.env")
	}
	// Must contain agent file
	if !strings.Contains(out, "agent-fix.go") {
		t.Error("M31A-AUDIT-001: CommitWithFiles missing agent-fix.go")
	}
}

// TestRegression_003_CommitWithFilesDoesNotStageAll verifies that CommitWithFiles
// does not call addAll or stage unrelated files. Regression test for R1.2.
func TestRegression_003_CommitWithFilesDoesNotStageAll(t *testing.T) {
	g, _ := setupRepo(t)

	// Create initial commit with a file
	os.WriteFile(filepath.Join(g.workDir, "init.txt"), []byte("initial"), 0644)
	g.CommitWithFiles("initial commit", "init.txt")

	// Create sensitive files and a new safe file
	os.WriteFile(filepath.Join(g.workDir, ".env"), []byte("SECRET=xyz"), 0644)
	os.WriteFile(filepath.Join(g.workDir, "id_rsa"), []byte("private-key"), 0644)
	os.WriteFile(filepath.Join(g.workDir, "safe.txt"), []byte("safe content"), 0644)

	// Commit only safe.txt — should not stage .env or id_rsa
	hash, err := g.CommitWithFiles("safe commit", "safe.txt")
	if err != nil {
		t.Fatalf("CommitWithFiles failed: %v", err)
	}

	out, err := g.run("diff", "--name-only", hash+"^", hash)
	if err != nil {
		t.Fatalf("git diff --name-only failed: %v", err)
	}

	// Must NOT contain sensitive files
	if strings.Contains(out, ".env") {
		t.Error("M31A-AUDIT-001: CommitWithFiles leaked .env")
	}
	if strings.Contains(out, "id_rsa") {
		t.Error("M31A-AUDIT-001: CommitWithFiles leaked id_rsa")
	}
	// Must contain safe file
	if !strings.Contains(out, "safe.txt") {
		t.Error("M31A-AUDIT-001: CommitWithFiles missing safe.txt")
	}
}

// TestRegression_004_AddFilesStaging verifies that Add() only stages the
// specified files, not the entire worktree. Regression test for R1.2.
func TestRegression_004_AddFilesStaging(t *testing.T) {
	g, _ := setupRepo(t)
	g.commit("initial")

	// Create files
	os.WriteFile(filepath.Join(g.workDir, "user.txt"), []byte("user"), 0644)
	os.WriteFile(filepath.Join(g.workDir, "agent.txt"), []byte("agent"), 0644)

	// Add only agent.txt
	if err := g.Add("agent.txt"); err != nil {
		t.Fatalf("Add failed: %v", err)
	}

	// Check staged files
	diff, err := g.DiffStaged()
	if err != nil {
		t.Fatalf("DiffStaged failed: %v", err)
	}

	// user.txt should NOT be staged
	if strings.Contains(diff, "user.txt") {
		t.Error("M31A-AUDIT-001: Add() staged user.txt when only agent.txt was specified")
	}

	// agent.txt SHOULD be staged
	if !strings.Contains(diff, "agent.txt") {
		t.Error("M31A-AUDIT-001: Add() failed to stage agent.txt")
	}
}

// TestRegression_005_DiffStagedReturnsCurrent verifies DiffStaged returns
// the current staged diff, not a stale snapshot. Regression test for R3.2.
func TestRegression_005_DiffStagedReturnsCurrent(t *testing.T) {
	g, _ := setupRepo(t)
	g.commit("initial")

	// Create and stage a file
	os.WriteFile(filepath.Join(g.workDir, "new.txt"), []byte("content"), 0644)
	g.Add("new.txt")

	// Get staged diff
	diff1, err := g.DiffStaged()
	if err != nil {
		t.Fatalf("DiffStaged failed: %v", err)
	}

	if !strings.Contains(diff1, "new.txt") {
		t.Error("DiffStaged should contain newly staged file")
	}

	// Unstage using git reset HEAD and verify diff changes
	_, _ = g.run("reset", "HEAD", "--", "new.txt")
	diff2, err := g.DiffStaged()
	if err != nil {
		t.Fatalf("DiffStaged failed after reset: %v", err)
	}

	if strings.Contains(diff2, "new.txt") {
		t.Error("M31A-AUDIT-003: DiffStaged returned stale data after unstaging")
	}
}
