package tui

import (
	"context"
	"fmt"
	"os"
	"strconv"
	"strings"
	"syscall"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/arbitrage"
)

// handleModel shows or switches the active model.
func handleModel(args []string, ctx CommandContext) CommandResult {
	if len(args) == 0 {
		msg := "No model selected. Use /model <name> to view model info, or /models to browse available models."
		if ctx.Config != nil && ctx.Config.Model.Default != "" {
			msg = fmt.Sprintf("Current default model: %s\n", ctx.Config.Model.Default) + msg
		}
		return CommandResult{Success: true, Message: msg}
	}

	if args[0] == "--selector" {
		screen := ScreenModelSelector
		return CommandResult{Success: true, Screen: &screen, Message: "Opening model selector..."}
	}

	if ctx.Registry != nil {
		ap := ctx.Registry.ActiveProvider()
		if ap != nil {
			modelInfo, err := ap.GetModel(args[0])
			if err != nil || modelInfo == nil {
				return CommandResult{
					Success: false,
					Message: fmt.Sprintf("Model %q not found in provider %q.", args[0], ctx.Registry.Active()),
				}
			}
			return CommandResult{
				Success: true,
				Message: fmt.Sprintf(
					"Model: %s\n  Provider:   %s\n  Context:    %d tokens\n  Pricing:    $%.4f / $%.4f (in/out per MTok)",
					modelInfo.ID, modelInfo.Provider, modelInfo.ContextLength,
					modelInfo.Pricing.InputPerMToken, modelInfo.Pricing.OutputPerMToken,
				),
			}
		}
	}

	return CommandResult{Success: false, Message: "No provider available to look up models."}
}

// handleProvider shows the active provider or switches to a different one.
func handleProvider(args []string, ctx CommandContext) CommandResult {
	if ctx.Registry == nil {
		return CommandResult{Success: false, Message: "Registry not available."}
	}

	if len(args) == 0 {
		active := ctx.Registry.Active()
		providers := strings.Join(ctx.Registry.List(), ", ")
		if active == "" {
			return CommandResult{Success: false, Message: "No active provider. Available: " + providers}
		}
		return CommandResult{Success: true, Message: fmt.Sprintf("Active provider: %s\nAvailable: %s", active, providers)}
	}

	if err := ctx.Registry.SetActive(args[0]); err != nil {
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("Cannot switch to provider %q: %v. Available: %s", args[0], err, strings.Join(ctx.Registry.List(), ", ")),
		}
	}

	return CommandResult{Success: true, Message: fmt.Sprintf("Switched to provider: %s", args[0])}
}

// handleConfig displays or modifies configuration values.
func handleConfig(args []string, ctx CommandContext) CommandResult {
	if ctx.Config == nil {
		return CommandResult{Success: false, Message: "Config not available."}
	}

	if len(args) == 0 {
		return formatConfig(ctx.Config)
	}

	if len(args) >= 2 {
		key := args[0]
		value := args[1]

		switch key {
		case "ui.theme":
			ctx.Config.UI.Theme = value
			ctx.Config.Save(ctx.ConfigPath)
			return CommandResult{Success: true, Message: fmt.Sprintf("ui.theme set to %q", value), Cmd: func() tea.Msg {
				return ThemeChangedMsg{Theme: value}
			}}
		case "model.default":
			ctx.Config.Model.Default = value
			ctx.Config.Save(ctx.ConfigPath)
			return CommandResult{Success: true, Message: fmt.Sprintf("model.default set to %q", value)}
		case "ui.compact_mode":
			if v, err := strconv.ParseBool(value); err == nil {
				ctx.Config.UI.CompactMode = v
				ctx.Config.Save(ctx.ConfigPath)
				return CommandResult{Success: true, Message: fmt.Sprintf("ui.compact_mode set to %v", v)}
			}
			return CommandResult{Success: false, Message: fmt.Sprintf("Invalid boolean value: %q", value)}
		case "ui.show_token_usage":
			if v, err := strconv.ParseBool(value); err == nil {
				ctx.Config.UI.ShowTokenUsage = v
				ctx.Config.Save(ctx.ConfigPath)
				return CommandResult{Success: true, Message: fmt.Sprintf("ui.show_token_usage set to %v", v)}
			}
			return CommandResult{Success: false, Message: fmt.Sprintf("Invalid boolean value: %q", value)}
		case "provider.auto_fallback":
			if v, err := strconv.ParseBool(value); err == nil {
				ctx.Config.Provider.AutoFallback = v
				ctx.Config.Save(ctx.ConfigPath)
				return CommandResult{Success: true, Message: fmt.Sprintf("provider.auto_fallback set to %v", v)}
			}
			return CommandResult{Success: false, Message: fmt.Sprintf("Invalid boolean value: %q", value)}
		case "ledger.enabled":
			if v, err := strconv.ParseBool(value); err == nil {
				ctx.Config.Ledger.Enabled = v
				ctx.Config.Save(ctx.ConfigPath)
				return CommandResult{Success: true, Message: fmt.Sprintf("ledger.enabled set to %v", v)}
			}
			return CommandResult{Success: false, Message: fmt.Sprintf("Invalid boolean value: %q", value)}
		case "permissions.mode":
			if value == "prompt" || value == "allow" || value == "deny" {
				ctx.Config.Permissions.DefaultMode = value
				ctx.Config.Save(ctx.ConfigPath)
				return CommandResult{Success: true, Message: fmt.Sprintf("permissions.mode set to %q", value)}
			}
			return CommandResult{Success: false, Message: fmt.Sprintf("Invalid value: %q. Use \"prompt\", \"allow\", or \"deny\"", value)}
		case "ui.show_cost_estimate":
			if v, err := strconv.ParseBool(value); err == nil {
				ctx.Config.UI.ShowCostEstimate = v
				ctx.Config.Save(ctx.ConfigPath)
				return CommandResult{Success: true, Message: fmt.Sprintf("ui.show_cost_estimate set to %v", v)}
			}
			return CommandResult{Success: false, Message: fmt.Sprintf("Invalid boolean value: %q", value)}
		case "model.auto_arbitrage":
			if v, err := strconv.ParseBool(value); err == nil {
				ctx.Config.Model.AutoArbitrage = v
				ctx.Config.Save(ctx.ConfigPath)
				return CommandResult{Success: true, Message: fmt.Sprintf("model.auto_arbitrage set to %v", v)}
			}
			return CommandResult{Success: false, Message: fmt.Sprintf("Invalid boolean value: %q", value)}
		default:
			return CommandResult{Success: false, Message: fmt.Sprintf("Unknown config key: %q. Settable keys: ui.theme, model.default, ui.compact_mode, ui.show_token_usage, ui.show_cost_estimate, provider.auto_fallback, ledger.enabled, permissions.mode, model.auto_arbitrage", key)}
		}
	}

	return CommandResult{Success: false, Message: "Usage: /config [key value]"}
}

func formatConfig(cfg *config.Config) CommandResult {
	var b strings.Builder
	b.WriteString("Configuration:\n")
	b.WriteString(fmt.Sprintf("  provider.default:       %s\n", cfg.Provider.Default))
	b.WriteString(fmt.Sprintf("  provider.auto_fallback: %v\n", cfg.Provider.AutoFallback))
	b.WriteString(fmt.Sprintf("  model.default:          %s\n", cfg.Model.Default))
	b.WriteString(fmt.Sprintf("  model.arbitrage:        %v\n", cfg.Model.AutoArbitrage))
	b.WriteString(fmt.Sprintf("  model.arbitrage_thresh: %.2f\n", cfg.Model.ArbitrageThreshold))
	b.WriteString(fmt.Sprintf("  ui.theme:               %s\n", cfg.UI.Theme))
	b.WriteString(fmt.Sprintf("  ui.compact_mode:        %v\n", cfg.UI.CompactMode))
	b.WriteString(fmt.Sprintf("  ui.show_token_usage:    %v\n", cfg.UI.ShowTokenUsage))
	b.WriteString(fmt.Sprintf("  ui.show_cost_estimate:  %v\n", cfg.UI.ShowCostEstimate))
	b.WriteString(fmt.Sprintf("  permissions.mode:       %s\n", cfg.Permissions.DefaultMode))
	b.WriteString(fmt.Sprintf("  ledger.enabled:         %v\n", cfg.Ledger.Enabled))
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleModels lists all cached models from the active provider.
func handleModels(args []string, ctx CommandContext) CommandResult {
	if ctx.Registry == nil {
		return CommandResult{Success: false, Message: "Provider registry not available."}
	}

	ap := ctx.Registry.ActiveProvider()
	if ap == nil {
		return CommandResult{Success: false, Message: "No active provider."}
	}

	models, err := ap.FetchModels(context.Background())
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to fetch models: %v", err)}
	}

	if len(models) == 0 {
		return CommandResult{Success: true, Message: "No models cached."}
	}

	var b strings.Builder
	b.WriteString(fmt.Sprintf("Cached models (%s):\n", ctx.Registry.Active()))
	for i, m := range models {
		if i >= 20 {
			b.WriteString(fmt.Sprintf("  ... and %d more\n", len(models)-20))
			break
		}
		b.WriteString(fmt.Sprintf("  %s | ctx=%d | $%.4f/$%.4f\n",
			m.ID, m.ContextLength, m.Pricing.InputPerMToken, m.Pricing.OutputPerMToken))
	}
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleFallback shows the active provider and available alternatives.
func handleFallback(args []string, ctx CommandContext) CommandResult {
	if ctx.Registry == nil {
		return CommandResult{Success: false, Message: "Provider registry not available."}
	}

	active := ctx.Registry.Active()
	providers := ctx.Registry.List()

	if len(args) == 0 {
		if active == "" {
			return CommandResult{Success: false, Message: "No active provider. Available: " + strings.Join(providers, ", ")}
		}
		var b strings.Builder
		b.WriteString(fmt.Sprintf("Active provider: %s\n", active))
		b.WriteString(fmt.Sprintf("Available: %s\n", strings.Join(providers, ", ")))
		if len(providers) > 1 {
			b.WriteString("Use /fallback <provider_name> to switch.")
		}
		return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
	}

	target := args[0]
	if target == active {
		return CommandResult{Success: false, Message: fmt.Sprintf("Already using provider %q.", target)}
	}

	valid := false
	for _, p := range providers {
		if p == target {
			valid = true
			break
		}
	}
	if !valid {
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("Provider %q not found. Available: %s", target, strings.Join(providers, ", ")),
		}
	}

	if err := ctx.Registry.SetActive(target); err != nil {
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("Cannot switch to provider %q: %v.", target, err),
		}
	}

	return CommandResult{Success: true, Message: fmt.Sprintf("Switched from %s to %s. Use /status to confirm.", active, target)}
}

// handleCost toggles the ShowCostEstimate config flag and persists to disk.
// M-23: the /cost command now actually flips the flag.
func handleCost(args []string, ctx CommandContext) CommandResult {
	if ctx.Config == nil {
		return CommandResult{Success: false, Message: "Config not loaded."}
	}

	ctx.Config.UI.ShowCostEstimate = !ctx.Config.UI.ShowCostEstimate

	if ctx.ConfigPath != "" {
		if err := ctx.Config.Save(ctx.ConfigPath); err != nil {
			return CommandResult{Success: false, Message: fmt.Sprintf("Failed to persist config: %v", err)}
		}
	}

	state := "disabled"
	if ctx.Config.UI.ShowCostEstimate {
		state = "enabled"
	}
	return CommandResult{Success: true, Message: fmt.Sprintf("Cost display: %s", state)}
}

// handleOptimize suggests cheaper model alternatives using the arbitrage engine.
// H-19/M-12/M-14: reads AutoArbitrage and ArbitrageThreshold from config.
func handleOptimize(args []string, ctx CommandContext) CommandResult {
	if ctx.Registry == nil {
		return CommandResult{Success: false, Message: "Provider registry not available."}
	}

	// M-12: respect AutoArbitrage config flag
	if ctx.Config != nil && !ctx.Config.Model.AutoArbitrage {
		return CommandResult{Success: false, Message: "AutoArbitrage is disabled in config. Set auto_arbitrage = true in [model] to enable."}
	}

	p := ctx.Registry.ActiveProvider()
	if p == nil {
		return CommandResult{Success: false, Message: "No active provider."}
	}

	allModels, err := p.FetchModels(context.Background())
	if err != nil || len(allModels) == 0 {
		return CommandResult{Success: false, Message: "Failed to fetch model catalog."}
	}

	currentID := ""
	if info, _ := p.GetModel(""); info != nil {
		currentID = info.ID
	}

	// M-14: read threshold from config
	threshold := 0.15
	if ctx.Config != nil && ctx.Config.Model.ArbitrageThreshold > 0 {
		threshold = ctx.Config.Model.ArbitrageThreshold
	}

	task := types.Task{Description: "general coding task", Files: []string{}}

	rec, err := arbitrage.Recommend(allModels, task, threshold)
	if err != nil || rec == nil {
		return CommandResult{Success: true, Message: "No cheaper alternatives found for general workloads."}
	}

	var b strings.Builder
	b.WriteString("Cost Optimization Suggestion:\n\n")
	if currentID != "" {
		b.WriteString(fmt.Sprintf("  Current:  %s\n", currentID))
	}
	b.WriteString(fmt.Sprintf("  Suggest:  %s\n", rec.RecommendedModel.ModelID))
	b.WriteString(fmt.Sprintf("  Savings:  $%.4f per request\n", rec.Savings))
	b.WriteString(fmt.Sprintf("  Reason:   %s complexity, %s\n", rec.Complexity, rec.Reason))
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleKey shows API key status and resolution source.
func handleKey(args []string, ctx CommandContext) CommandResult {
	if ctx.Config == nil {
		return CommandResult{Success: false, Message: "Config not available."}
	}

	var b strings.Builder
	b.WriteString("API Key Status:\n")

	if ctx.Config.Provider.OpenRouter.APIKey != "" {
		key := ctx.Config.Provider.OpenRouter.APIKey
		masked := "***" + key[len(key)-4:]
		b.WriteString(fmt.Sprintf("  OpenRouter:  %s (from config)\n", masked))
	} else {
		b.WriteString("  OpenRouter:  not set\n")
	}

	if ctx.Config.Provider.Zen.APIKey != "" {
		key := ctx.Config.Provider.Zen.APIKey
		masked := "***" + key[len(key)-4:]
		b.WriteString(fmt.Sprintf("  Zen:         %s (from config)\n", masked))
	} else {
		b.WriteString("  Zen:         not set\n")
	}

	b.WriteString("\nKey resolution order: environment variable → OS keychain → config file")
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleHealth shows system health status.
func handleHealth(args []string, ctx CommandContext) CommandResult {
	var b strings.Builder
	b.WriteString("Health Status:\n")

	if ctx.Registry != nil {
		active := ctx.Registry.Active()
		if active != "" {
			b.WriteString(fmt.Sprintf("  Provider:    %s (active)\n", active))
		} else {
			b.WriteString("  Provider:    none configured\n")
		}
	}

	if ctx.SessionManager != nil && ctx.SessionID != "" {
		b.WriteString(fmt.Sprintf("  Session:     %s (active)\n", ctx.SessionID))
	} else {
		b.WriteString("  Session:     none active\n")
	}

	if ctx.Git != nil {
		b.WriteString("  Git:         available\n")
	} else {
		b.WriteString("  Git:         not available\n")
	}

	if ctx.Dispatcher != nil {
		toolCount := len(ctx.Dispatcher.List())
		b.WriteString(fmt.Sprintf("  Tools:       %d registered\n", toolCount))
	}

	home, err := os.UserHomeDir()
	if err == nil {
		var statfs syscall.Statfs_t
		if err := syscall.Statfs(home, &statfs); err == nil {
			availGB := float64(statfs.Bavail*uint64(statfs.Bsize)) / 1e9
			b.WriteString(fmt.Sprintf("  Disk:        %.1f GB available\n", availGB))
		}
	}

	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}
