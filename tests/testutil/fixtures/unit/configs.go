// Package unit provides type-safe, compile-time embedded test fixtures for unit tests.
package unit

import _ "embed"

// MinimalConfig is a minimal TOML configuration for unit tests.
//
//go:embed minimal.toml
var MinimalConfig string

// FullConfig is a complete TOML configuration for unit tests.
//
//go:embed full.toml
var FullConfig string
