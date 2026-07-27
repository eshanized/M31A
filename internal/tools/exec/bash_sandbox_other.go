//go:build !linux && !darwin && !windows

package exec

import (
	"log/slog"
	"os"
	"os/exec"
	"strings"
)

// applyBashSandbox applies environment scrubbing to a bash subprocess on
// platforms where OS-level sandboxing is not available. This is the minimum
// sandboxing provided on all platforms: sensitive environment variables are
// removed before the subprocess is started.
func applyBashSandbox(cmd *exec.Cmd, workDir string) error {
	ScrubEnvironment(cmd)

	slog.Debug("platform sandbox not available, using env-scrub-only mode")
	return nil
}

// ScrubEnvironment removes sensitive environment variables from the subprocess.
func ScrubEnvironment(cmd *exec.Cmd) {
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
				if strings.HasPrefix(strings.ToUpper(key), strings.ToUpper(prefix)) || strings.HasSuffix(strings.ToUpper(key), strings.ToUpper(prefix)) {
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
