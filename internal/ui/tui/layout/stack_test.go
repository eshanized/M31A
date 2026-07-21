package layout

import (
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

func TestRenderModalOverlay_PreservesANSI(t *testing.T) {
	tm := theme.Dark()
	base := "\x1b[31mred text\x1b[0m\n\x1b[32mgreen text\x1b[0m\n\x1b[34mblue text\x1b[0m"
	modal := "Hello"

	result := RenderModalOverlay(base, modal, 40, 5, tm)

	if !strings.Contains(result, "Hello") {
		t.Errorf("expected modal text 'Hello' in output, got:\n%s", result)
	}
	if strings.Contains(result, "\x00") {
		t.Error("output contains null bytes (corrupted ANSI)")
	}

	lines := strings.Split(result, "\n")
	if len(lines) != 5 {
		t.Errorf("expected 5 lines, got %d", len(lines))
	}
}

func TestRenderModalOverlay_EmptyBase(t *testing.T) {
	tm := theme.Dark()
	modal := "Modal Content"

	result := RenderModalOverlay("", modal, 60, 10, tm)

	if !strings.Contains(result, "Modal Content") {
		t.Errorf("expected modal text in output, got:\n%s", result)
	}

	lines := strings.Split(result, "\n")
	if len(lines) != 10 {
		t.Errorf("expected 10 lines, got %d", len(lines))
	}
}

func TestRenderModalOverlay_StyledModal(t *testing.T) {
	tm := theme.Dark()
	base := strings.Repeat("plain text\n", 10)
	modal := "\x1b[1;34m╔══════════╗\x1b[0m\n\x1b[1;34m║ Modal    ║\x1b[0m\n\x1b[1;34m╚══════════╝\x1b[0m"

	result := RenderModalOverlay(base, modal, 60, 10, tm)

	if !strings.Contains(result, "Modal") {
		t.Errorf("expected modal text in output, got:\n%s", result)
	}
	if !strings.Contains(result, "\x1b[1;34m") {
		t.Error("expected modal ANSI styling to be preserved")
	}
}

func TestRenderModalOverlay_ModalCentered(t *testing.T) {
	tm := theme.Dark()
	base := ""
	modal := "X"

	result := RenderModalOverlay(base, modal, 20, 5, tm)

	lines := strings.Split(result, "\n")
	modalLine := lines[2]

	if !strings.Contains(modalLine, "X") {
		t.Error("expected modal on center row")
	}
}
