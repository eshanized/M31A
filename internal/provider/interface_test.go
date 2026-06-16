package provider

import (
	"testing"

	"github.com/eshanized/M31A/internal/types"
)

func TestLLMProvider_Interface_Exists(t *testing.T) {
	t.Parallel()
	// Verify the interface exists and can be referenced
	var _ LLMProvider = (LLMProvider)(nil)
	_ = types.HealthStatus{}
}
