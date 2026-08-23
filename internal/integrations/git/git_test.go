package git

import (
	"os"
	"path/filepath"
	"testing"
)

func setupRepo(t *testing.T) (*Git, string) {
	t.Helper()
	dir := t.TempDir()
	g := New(dir)
	if err := g.Init(); err != nil {
		t.Fatalf("Init failed: %v", err)
	}
	if err := g.ConfigUser("Test User", "test@test.com"); err != nil {
		t.Fatalf("ConfigUser failed: %v", err)
	}
	return g, dir
}

func TestGit_Init(t *testing.T) {
	dir := t.TempDir()
	g := New(dir)

	if err := g.Init(); err != nil {
		t.Fatalf("Init failed: %v", err)
	}

	if !g.IsRepo() {
		t.Fatal("Expected IsRepo to return true after Init")
	}
}

func TestGit_IsRepo(t *testing.T) {
	dir := t.TempDir()
	g := New(dir)

	if g.IsRepo() {
		t.Fatal("Expected IsRepo to return false for non-git directory")
	}

	if err := g.Init(); err != nil {
		t.Fatalf("Init failed: %v", err)
	}

	if !g.IsRepo() {
		t.Fatal("Expected IsRepo to return true after Init")
	}
}

func TestGit_AddAndCommit(t *testing.T) {
	g, _ := setupRepo(t)

	// Create a file
	if err := os.WriteFile(filepath.Join(g.workDir, "test.txt"), []byte("hello"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	// Commit
	if err := g.commit("initial commit"); err != nil {
		t.Fatalf("Commit failed: %v", err)
	}

	// Verify HEAD exists
	hash, err := g.HeadHash()
	if err != nil {
		t.Fatalf("HeadHash failed: %v", err)
	}
	if hash == "" {
		t.Fatal("Expected non-empty HEAD hash")
	}
}

func TestGit_CommitWithFiles(t *testing.T) {
	g, _ := setupRepo(t)

	// Create two files
	file1 := filepath.Join(g.workDir, "a.txt")
	file2 := filepath.Join(g.workDir, "b.txt")
	if err := os.WriteFile(file1, []byte("a"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}
	if err := os.WriteFile(file2, []byte("b"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	// Commit only a.txt
	if _, err := g.CommitWithFiles("add a", "a.txt"); err != nil {
		t.Fatalf("CommitWithFiles failed: %v", err)
	}

	// Verify b.txt is not committed
	status, err := g.Status()
	if err != nil {
		t.Fatalf("Status failed: %v", err)
	}
	if status == "" {
		t.Fatal("Expected status to show unstaged files")
	}
}

func TestGit_Log(t *testing.T) {
	g, _ := setupRepo(t)

	// Create 3 commits
	for i := 0; i < 3; i++ {
		f := filepath.Join(g.workDir, "file"+string(rune('0'+i))+".txt")
		os.WriteFile(f, []byte("content"), 0644)
		if err := g.commit("commit " + string(rune('0'+i))); err != nil {
			t.Fatalf("Commit %d failed: %v", i, err)
		}
	}

	commits, err := g.LogAll()
	if err != nil {
		t.Fatalf("Log failed: %v", err)
	}
	if len(commits) != 3 {
		t.Fatalf("Expected 3 commits, got %d", len(commits))
	}

	// Verify order (newest first)
	if commits[0].Message != "commit 2" {
		t.Errorf("Expected newest commit first, got %q", commits[0].Message)
	}
}

func TestGit_LogEmpty(t *testing.T) {
	g, _ := setupRepo(t)

	commits, err := g.LogAll()
	if err != nil {
		t.Fatalf("LogAll on empty repo should not error, got: %v", err)
	}
	if len(commits) != 0 {
		t.Errorf("Expected 0 commits, got %d", len(commits))
	}
}

func TestGit_Diff(t *testing.T) {
	g, _ := setupRepo(t)

	// First commit
	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("v1"), 0644)
	g.commit("v1")
	hash1, _ := g.HeadHash()

	// Second commit modifies
	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("v2"), 0644)
	g.commit("v2")
	hash2, _ := g.HeadHash()

	diff, err := g.DiffRefs(hash1, hash2)
	if err != nil {
		t.Fatalf("Diff failed: %v", err)
	}
	if diff == "" {
		t.Fatal("Expected non-empty diff between two different commits")
	}
}

func TestGit_HeadHash(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "x.txt"), []byte("x"), 0644)
	g.commit("test")

	hash, err := g.HeadHash()
	if err != nil {
		t.Fatalf("HeadHash failed: %v", err)
	}
	if len(hash) != 40 {
		t.Errorf("Expected 40 char hash, got %d chars", len(hash))
	}
}

func TestGit_CreateBranch(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "x.txt"), []byte("x"), 0644)
	g.commit("initial")

	if err := g.CreateBranch("feature"); err != nil {
		t.Fatalf("CreateBranch failed: %v", err)
	}

	// Branch exists if we can show it in branch list
	status, _ := g.Status()
	_ = status // just verifying no error
}

func TestGit_CurrentBranch(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "x.txt"), []byte("x"), 0644)
	g.commit("initial")

	branch, err := g.CurrentBranch()
	if err != nil {
		t.Fatalf("CurrentBranch failed: %v", err)
	}
	if branch == "" {
		t.Fatal("Expected non-empty branch name")
	}
}

func TestGit_Status(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "untracked.txt"), []byte("data"), 0644)

	status, err := g.Status()
	if err != nil {
		t.Fatalf("Status failed: %v", err)
	}
	if status == "" {
		t.Fatal("Expected non-empty status output")
	}
}

func TestGit_ResetSoft(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("v1"), 0644)
	g.commit("commit 1")
	hash1, _ := g.HeadHash()

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("v2"), 0644)
	g.commit("commit 2")

	if err := g.ResetSoft(hash1); err != nil {
		t.Fatalf("ResetSoft failed: %v", err)
	}

	head, _ := g.HeadHash()
	if head != hash1 {
		t.Errorf("Expected HEAD at %s, got %s", hash1, head)
	}
}

func TestGit_ResetHard(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("v1"), 0644)
	g.commit("commit 1")
	hash1, _ := g.HeadHash()

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("v2"), 0644)
	g.commit("commit 2")

	if err := g.ResetHard(hash1); err != nil {
		t.Fatalf("ResetHard failed: %v", err)
	}

	head, _ := g.HeadHash()
	if head != hash1 {
		t.Errorf("Expected HEAD at %s, got %s", hash1, head)
	}

	// File should be back to v1
	content, _ := os.ReadFile(filepath.Join(g.workDir, "a.txt"))
	if string(content) != "v1" {
		t.Errorf("Expected file content 'v1', got %q", string(content))
	}
}

func TestGit_Stash(t *testing.T) {
	g, _ := setupRepo(t)

	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("v1"), 0644)
	g.commit("initial")

	// Modify file
	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("modified"), 0644)

	if err := g.StashPush("temp changes"); err != nil {
		t.Fatalf("StashPush failed: %v", err)
	}

	// File should be back to v1
	content, _ := os.ReadFile(filepath.Join(g.workDir, "a.txt"))
	if string(content) != "v1" {
		t.Errorf("Expected file content 'v1' after stash, got %q", string(content))
	}

	if err := g.StashPop(); err != nil {
		t.Fatalf("StashPop failed: %v", err)
	}

	content, _ = os.ReadFile(filepath.Join(g.workDir, "a.txt"))
	if string(content) != "modified" {
		t.Errorf("Expected file content 'modified' after stash pop, got %q", string(content))
	}
}

func TestGit_AddAll(t *testing.T) {
	g, _ := setupRepo(t)

	// Create and delete a file
	os.WriteFile(filepath.Join(g.workDir, "a.txt"), []byte("a"), 0644)
	os.WriteFile(filepath.Join(g.workDir, "b.txt"), []byte("b"), 0644)
	g.commit("initial")

	os.Remove(filepath.Join(g.workDir, "a.txt"))
	os.WriteFile(filepath.Join(g.workDir, "c.txt"), []byte("c"), 0644)

	if err := g.addAll(); err != nil {
		t.Fatalf("addAll failed: %v", err)
	}
}

func TestGit_WorkDir(t *testing.T) {
	dir := t.TempDir()
	g := New(dir)

	if g.WorkDir() != dir {
		t.Errorf("Expected WorkDir to return %q, got %q", dir, g.WorkDir())
	}
}
