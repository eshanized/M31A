//go:build windows

package tools

import (
	"log/slog"
	"os"
	"os/exec"
	"strings"
)

// applyBashSandbox applies OS-level sandboxing to a bash subprocess on Windows.
// It scrubs sensitive environment variables and sets up a restricted process
// environment. Full sandboxing (restricted tokens, job objects) requires CGO
// for Windows API access; this provides the maximum sandboxing achievable
// without CGO.
func applyBashSandbox(cmd *exec.Cmd, workDir string) error {
	// Scrub sensitive environment variables from the subprocess
	scrubEnvironment(cmd)

	// Set restricted working directory if specified
	if workDir != "" {
		cmd.Dir = workDir
	}

	slog.Debug("windows sandbox: env-scrub-only mode (full sandboxing requires CGO)")
	return nil
}

// scrubEnvironment removes sensitive environment variables from the subprocess.
func scrubEnvironment(cmd *exec.Cmd) {
	env := os.Environ()

	sensitiveVars := []string{
		"OPENROUTER_API_KEY",
		"ZEN_API_KEY",
		"NVIDIA_API_KEY",
		"ANTHROPIC_API_KEY",
		"OPENAI_API_KEY",
		"GITHUB_TOKEN",
		"GITHUB_SECRET",
		"AWS_SECRET_ACCESS_KEY",
		"AWS_SESSION_TOKEN",
		"GCP_SERVICE_ACCOUNT_KEY",
		"AZURE_CLIENT_SECRET",
		"DATABASE_URL",
		"REDIS_URL",
		"SECRET_KEY",
		"PRIVATE_KEY",
	}

	sensitivePrefixes := []string{
		"API_KEY",
		"TOKEN",
		"SECRET",
		"PASSWORD",
		"CREDENTIAL",
		"AUTH",
	}

	scrubbed := make([]string, 0, len(env))
	for _, e := range env {
		key := e
		if idx := strings.IndexByte(e, '='); idx >= 0 {
			key = e[:idx]
		}

		sensitive := false
		for _, sv := range sensitiveVars {
			if key == sv {
				sensitive = true
				break
			}
		}

		if !sensitive {
			for _, prefix := range sensitivePrefixes {
				if strings.HasPrefix(strings.ToUpper(key), strings.ToUpper(prefix)) {
					sensitive = true
					break
				}
			}
		}

		if !sensitive {
			scrubbed = append(scrubbed, e)
		}
	}

	scrubbed = append(scrubbed,
		"CI=true",
		"DEBIAN_FRONTEND=noninteractive",
		"npm_config_yes=true",
		"PIP_NO_INPUT=1",
		"YARN_ENABLE_IMMUTABLE_INSTALLS=false",
	)

	cmd.Env = scrubbed
}
