package lsp

import (
	"encoding/json"
)

// LSPCapabilities represents the negotiated capabilities of a language server.
type LSPCapabilities struct {
	SupportsDefinition     bool
	SupportsReferences     bool
	SupportsCallHierarchy  bool
	SupportsTypeHierarchy  bool
	SupportsHover          bool
	SupportsCompletion     bool
}

// NegotiateCapabilities parses the server capabilities from the initialize response
// and returns an LSPCapabilities struct with boolean fields set based on what the server supports.
func NegotiateCapabilities(serverCaps json.RawMessage) LSPCapabilities {
	caps := LSPCapabilities{}

	if len(serverCaps) == 0 {
		return caps
	}

	var raw map[string]json.RawMessage
	if err := json.Unmarshal(serverCaps, &raw); err != nil {
		return caps
	}

	// Check definition provider
	if cap, ok := raw["definitionProvider"]; ok {
		caps.SupportsDefinition = isTruthy(cap)
	}

	// Check references provider
	if cap, ok := raw["referencesProvider"]; ok {
		caps.SupportsReferences = isTruthy(cap)
	}

	// Check call hierarchy provider
	if cap, ok := raw["callHierarchyProvider"]; ok {
		caps.SupportsCallHierarchy = isTruthy(cap)
	}

	// Check type hierarchy provider
	if cap, ok := raw["typeHierarchyProvider"]; ok {
		caps.SupportsTypeHierarchy = isTruthy(cap)
	}

	// Check hover provider
	if cap, ok := raw["hoverProvider"]; ok {
		caps.SupportsHover = isTruthy(cap)
	}

	// Check completion provider
	if cap, ok := raw["completionProvider"]; ok {
		caps.SupportsCompletion = isTruthy(cap)
	}

	return caps
}

// isTruthy checks if a JSON value represents a truthy value.
// According to LSP spec, the presence of a capability key (even with empty object)
// means the capability is supported.
func isTruthy(raw json.RawMessage) bool {
	var v interface{}
	if err := json.Unmarshal(raw, &v); err != nil {
		return false
	}

	switch val := v.(type) {
	case bool:
		return val
	case map[string]interface{}:
		// In LSP, even an empty object means the capability is supported
		return true
	case nil:
		return false
	default:
		// Any other non-null value (number, string, array) means supported
		return true
	}
}