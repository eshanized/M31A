package tools

import (
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
)

// BuildToolDefs creates provider.ToolDefinition slices from a Dispatcher's
// registered tools. This is shared between the workflow engine and the
// autonomous agent loop to avoid duplicating tool schema logic.
func BuildToolDefs(d *Dispatcher) []provider.ToolDefinition {
	var defs []provider.ToolDefinition
	for _, name := range d.List() {
		tool, ok := d.GetTool(name)
		if !ok {
			continue
		}
		def := provider.ToolDefinition{
			Name:        tool.Name(),
			Description: tool.Description(),
			Parameters:  "{}",
		}
		if sp, ok := tool.(types.SchemaProvider); ok {
			def.Parameters = sp.ParameterSchema()
		}
		defs = append(defs, def)
	}
	return defs
}
