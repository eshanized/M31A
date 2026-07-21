// Package integration provides type-safe, compile-time embedded test fixtures for integration tests.
package integration

import _ "embed"

// PlanSample is a sample plan markdown for integration tests.
//
//go:embed plan_sample.md
var PlanSample string

// TasksJSON is a sample task list for integration tests.
//
//go:embed tasks.json
var TasksJSON []byte
