// Package tui implements the Bubble Tea TUI for M31A.
//
// This file re-exports command types from the commands sub-package so that
// existing code within the tui package can reference them without explicit
// imports. New code should import the commands package directly.
package tui

import "github.com/eshanized/M31A/internal/tui/commands"

// Command type re-exports
type (
	CommandInfo     = commands.CommandInfo
	CommandResult   = commands.CommandResult
	CommandHandler  = commands.CommandHandler
	CommandContext  = commands.CommandContext
	CommandRegistry = commands.CommandRegistry
)

// Command constructor re-exports
var (
	NewCommandRegistry = commands.NewCommandRegistry
	DefaultCommands    = commands.DefaultCommands
	ParseCommand       = commands.ParseCommand
)
