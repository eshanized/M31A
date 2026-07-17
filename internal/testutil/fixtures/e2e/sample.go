// Package e2e provides type-safe, compile-time embedded test fixtures for e2e tests.
package e2e

import _ "embed"

// PromptSimple is a simple prompt for e2e tests.
//
//go:embed prompt_simple.txt
var PromptSimple string
