package types

import (
	"encoding/json"
)

func MarshalEventPayload(v any) (json.RawMessage, error) {
	return json.Marshal(v)
}

func UnmarshalEventPayload[T any](data json.RawMessage) (T, error) {
	var v T
	if len(data) == 0 {
		return v, nil
	}
	err := json.Unmarshal(data, &v)
	return v, err
}