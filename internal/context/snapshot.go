package context

import "encoding/json"

// Snapshot holds a JSON-encoded state map for change detection.
type Snapshot struct {
	State map[string]string `json:"state"`
}

// NewSnapshot creates a Snapshot from a state map.
func NewSnapshot(state map[string]string) *Snapshot {
	return &Snapshot{State: state}
}

// Encode serializes the snapshot to JSON.
func (s *Snapshot) Encode() ([]byte, error) {
	return json.Marshal(s.State)
}

// DecodeSnapshot deserializes a snapshot from JSON.
func DecodeSnapshot(data []byte) (map[string]string, error) {
	var state map[string]string
	if err := json.Unmarshal(data, &state); err != nil {
		return nil, err
	}
	return state, nil
}
