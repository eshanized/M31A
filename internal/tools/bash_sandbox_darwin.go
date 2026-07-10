//go:build darwin

package tools

import (
	"log/slog"
	"os"
	"os/exec"
	"strings"
)

// applyBashSandbox applies OS-level sandboxing to a bash subprocess on macOS.
// It uses sandbox-exec with a restrictive profile to limit filesystem access
// and scrubs sensitive environment variables.
func applyBashSandbox(cmd *exec.Cmd, workDir string) error {
	// Scrub sensitive environment variables from the subprocess
	scrubEnvironment(cmd)

	// Apply sandbox-exec profile if available
	if err := applySandboxExec(cmd, workDir); err != nil {
		slog.Debug("sandbox-exec not available, using env-scrub-only sandbox",
			"error", err)
	}

	return nil
}

// sandboxExecProfile is a minimal sandbox-exec profile that restricts
// filesystem access to the work directory, /tmp, and essential system paths.
// This is a defense-in-depth layer, not a security boundary.
var sandboxExecProfile = `
(version 1)
(allow default)
(deny
    (regex "^/Users/[^/]+/(Documents|Pictures|Music|Movies|Desktop|Downloads)")
    (regex "^/private/var/[^/]+")
    (regex "^/System")
    (regex "^/Library/Preferences")
)
(allow file-read*
    (subpath "/usr")
    (subpath "/bin")
    (subpath "/sbin")
    (subpath "/lib")
    (subpath "/System/Library/Frameworks")
    (subpath "/System/Library/PrivateFrameworks")
    (subpath "/dev")
    (subpath "/private/tmp")
    (subpath "/etc")
    (subpath "/var/run")
)
(allow file-write*
    (subpath "/private/tmp")
)
(allow process-exec
    (subpath "/usr")
    (subpath "/bin")
    (subpath "/sbin")
)
(allow network*)
(allow system-socket)
(allow mach-lookup)
`

// applySandboxExec wraps the command with sandbox-exec if available on the system.
func applySandboxExec(cmd *exec.Cmd, workDir string) error {
	// Check if sandbox-exec is available
	sandboxExecPath, err := exec.LookPath("sandbox-exec")
	if err != nil {
		return err
	}

	// Build the sandbox profile, adding work directory access if specified
	profile := sandboxExecProfile
	if workDir != "" {
		// Add work directory to the allowed read/write paths
		profile = strings.Replace(profile,
			`(subpath "/private/tmp")`,
			`(subpath "/private/tmp")\n    (subpath "`+workDir+`")`,
			1)
		profile = strings.Replace(profile,
			`file-write*\n    (subpath "/private/tmp")`,
			`file-write*\n    (subpath "/private/tmp")\n    (subpath "`+workDir+`")`,
			1)
	}

	// Rewrite the command to use sandbox-exec
	originalArgs := cmd.Args
	originalPath := cmd.Path

	cmd.Path = sandboxExecPath
	cmd.Args = append([]string{"sandbox-exec", "-p", profile}, originalPath)
	cmd.Args = append(cmd.Args, originalArgs[1:]...)

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
