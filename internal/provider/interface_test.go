package provider

import (
	"testing"

	"github.com/eshanized/M31A/pkg/types"
)

func TestLLMProvider_Interface_Exists(t *testing.T) {
	t.Parallel()
	// Verify the interface exists and can be referenced
	var _ LLMProvider
	_ = types.HealthStatus{}
}
