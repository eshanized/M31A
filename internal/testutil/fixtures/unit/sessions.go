package unit

import _ "embed"

// SessionTemplate is a JSON session template for unit tests.
//
//go:embed session_template.json
var SessionTemplate []byte
