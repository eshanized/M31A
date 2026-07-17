package fileops

import (
	"github.com/eshanized/M31A/internal/types"
)

// Re-export constants from internal/types to avoid duplication.
// These were previously defined in pkg/tools/constants.go.
const (
	MaxBackupsPerFile    = types.DefaultMaxBackupsPerFile
	DirPermission        = types.DirPermission
	FilePermission       = types.FilePermission
	MinLinesForFuzzy     = 3
	LevenshteinThreshold = 0.7
)
