// Package taskrunner schedules and executes tasks with dependency resolution.
// It performs topological sorting to determine execution order, tracks task
// status, and supports self-healing retries for failed tasks.
package taskrunner
