package types

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestParseConfidence(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		want    Confidence
		wantErr bool
	}{
		{name: "verified", input: "verified", want: ConfidenceVerified},
		{name: "likely", input: "likely", want: ConfidenceLikely},
		{name: "speculative", input: "speculative", want: ConfidenceSpeculative},
		{name: "capitalized rejected", input: "Verified", wantErr: true},
		{name: "empty rejected", input: "", wantErr: true},
		{name: "unknown rejected", input: "certain", wantErr: true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := ParseConfidence(tt.input)
			if tt.wantErr {
				require.Error(t, err)
				// Error message must name the valid values.
				assert.Contains(t, err.Error(), "verified")
				assert.Contains(t, err.Error(), "likely")
				assert.Contains(t, err.Error(), "speculative")
				return
			}
			require.NoError(t, err)
			assert.Equal(t, tt.want, got)
		})
	}
}

func TestConfidenceRoundTrip(t *testing.T) {
	for _, c := range []Confidence{ConfidenceVerified, ConfidenceLikely, ConfidenceSpeculative} {
		parsed, err := ParseConfidence(string(c))
		require.NoError(t, err)
		assert.Equal(t, c, parsed)
		assert.True(t, parsed.Valid())
	}
}

func TestConfidenceValid(t *testing.T) {
	assert.True(t, ConfidenceVerified.Valid())
	assert.True(t, ConfidenceLikely.Valid())
	assert.True(t, ConfidenceSpeculative.Valid())
	assert.False(t, Confidence("").Valid())
	assert.False(t, Confidence("Verified").Valid())
	assert.False(t, Confidence("guess").Valid())
}

func TestConfidenceEnumValues(t *testing.T) {
	// D-07: the shared vocabulary exposes exactly three lowercase values.
	assert.Equal(t, "verified", string(ConfidenceVerified))
	assert.Equal(t, "likely", string(ConfidenceLikely))
	assert.Equal(t, "speculative", string(ConfidenceSpeculative))
}
