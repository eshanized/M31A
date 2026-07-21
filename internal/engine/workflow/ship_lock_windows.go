//go:build windows

package workflow

import (
	"os"
)

func flockWrite(f *os.File, data string) error {
	_, err := f.WriteString(data)
	return err
}
