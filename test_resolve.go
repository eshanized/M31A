package main
import (
	"fmt"
	"path/filepath"
    "strings"
)

func ContainedInWorkDir(resolved, workDir string) error {
	workDirPrefix := workDir
	if !strings.HasSuffix(workDirPrefix, string(filepath.Separator)) {
		workDirPrefix += string(filepath.Separator)
	}
	if resolved != workDir && !strings.HasPrefix(resolved, workDirPrefix) {
		return fmt.Errorf("path resolves outside working directory: resolved %q, workDir %q", resolved, workDir)
	}
	return nil
}

func main() {
    // macOS typical tmp directory structure
    workDir := "/var/folders/xx/yyyy/T/test1234"
    resolved := "/private/var/folders/xx/yyyy/T/test1234/main.go"

    err := ContainedInWorkDir(resolved, workDir)
    fmt.Printf("err: %v\n", err)
}
