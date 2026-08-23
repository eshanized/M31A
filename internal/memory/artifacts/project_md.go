package artifacts

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"

	coreerrors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
)

var (
	ErrProjectNotFound = errors.New("project not found")
)

func WriteProjectMD(m31aDir string, project *types.Project) error {
	if project == nil {
		return coreerrors.Wrap(ErrProjectNotFound, "project is nil")
	}

	data, err := json.MarshalIndent(project, "", "  ")
	if err != nil {
		return coreerrors.Wrap(err, "marshal project")
	}

	path := filepath.Join(m31aDir, "project.md")

	// Validate path is within workspace (prevent symlink escape)
	absPath, err := filepath.EvalSymlinks(path)
	if err != nil && !errors.Is(err, os.ErrNotExist) {
		return coreerrors.Wrap(err, "resolve path")
	}
	if err == nil {
		if !types.ValidatePath(absPath) {
			return errors.New("symlink escape detected: path outside workspace")
		}
	}

	// Atomic write: write to temp file in same directory, then rename
	if err := types.AtomicWrite(path, data); err != nil {
		return coreerrors.Wrap(err, "write project.md")
	}
	return nil
}

func ReadProjectMD(m31aDir string) (*types.Project, error) {
	path := filepath.Join(m31aDir, "project.md")

	fileData, err := os.ReadFile(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil, coreerrors.Wrap(ErrProjectNotFound, "project.md not found")
		}
		return nil, coreerrors.Wrap(err, "read project.md")
	}

	var project types.Project
	if err := json.Unmarshal(fileData, &project); err != nil {
		return nil, coreerrors.Wrap(err, "unmarshal project")
	}
	return &project, nil
}