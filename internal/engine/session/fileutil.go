//go:build !windows

package session

import "github.com/eshanized/M31A/internal/types"

// fileLock is a type alias for pkg/types.FileLock.
type fileLock = types.FileLock

// newFileLock creates a file lock for the given path.
var newFileLock = types.NewFileLock

// atomicWrite writes data to path atomically.
var atomicWrite = types.AtomicWrite
