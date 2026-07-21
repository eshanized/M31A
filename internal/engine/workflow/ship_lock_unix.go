//go:build !windows

package workflow

import (
	"os"
	"syscall"
)

func flockWrite(f *os.File, data string) error {
	if err := syscall.Flock(int(f.Fd()), syscall.LOCK_EX); err != nil {
		return err
	}
	defer func() {
		_ = syscall.Flock(int(f.Fd()), syscall.LOCK_UN)
	}()
	_, err := f.WriteString(data)
	return err
}
