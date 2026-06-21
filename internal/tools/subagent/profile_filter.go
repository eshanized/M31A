package subagent

// ApplyToolFilter removes tools from a ToolDispatcher based on the profile's
// AllowedTools and DeniedTools. If AllowedTools is non-empty, only listed
// tools are kept. DeniedTools are then removed from whatever remains.
func ApplyToolFilter(dispatcher ToolDispatcher, profile AgentProfile) {
	if len(profile.AllowedTools) == 0 && len(profile.DeniedTools) == 0 {
		return
	}

	allowed := make(map[string]bool, len(profile.AllowedTools))
	for _, name := range profile.AllowedTools {
		allowed[name] = true
	}
	denied := make(map[string]bool, len(profile.DeniedTools))
	for _, name := range profile.DeniedTools {
		denied[name] = true
	}

	for _, desc := range dispatcher.ListTools() {
		keep := true
		if len(allowed) > 0 && !allowed[desc.Name] {
			keep = false
		}
		if denied[desc.Name] {
			keep = false
		}
		if !keep {
			dispatcher.UnregisterTool(desc.Name)
		}
	}
}
