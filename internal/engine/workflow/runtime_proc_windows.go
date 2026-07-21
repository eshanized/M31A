//go:build windows

package workflow

import (
	"fmt"
	"os"
)

func getProcessGroup(pid int) (int, error) {
	return 0, fmt.Errorf("process groups not supported on Windows")
}

func killProcessGroup(pgid int) error {
	p, err := os.FindProcess(-pgid)
	if err != nil {
		return err
	}
	return p.Kill()
}
