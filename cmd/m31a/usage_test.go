package main

import (
	"bytes"
	"os"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/ui/tui"
)

func TestPrintUsage_ContainsNVIDIAKey(t *testing.T) {
	registry := tui.NewCommandRegistry()

	// Capture stderr
	old := os.Stderr
	r, w, _ := os.Pipe()
	os.Stderr = w

	printUsage(registry)

	w.Close()
	os.Stderr = old

	var buf bytes.Buffer
	buf.ReadFrom(r)
	output := buf.String()

	if !strings.Contains(output, "M31A_NVIDIA_API_KEY") {
		t.Error("printUsage output should contain M31A_NVIDIA_API_KEY")
	}
	if !strings.Contains(output, "NVIDIA API key") {
		t.Error("printUsage output should contain description 'NVIDIA API key'")
	}
}
