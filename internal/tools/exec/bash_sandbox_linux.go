//go:build linux

package exec

import (
	"fmt"
	"log/slog"
	"os"
	"os/exec"
	"syscall"
	"unsafe"

	"golang.org/x/sys/unix"
)

// applyBashSandbox applies OS-level sandboxing to a bash subprocess on Linux.
// It uses Landlock (when available) to restrict filesystem access and scrubs
// sensitive environment variables. On kernels before 5.13 or when Landlock is
// unavailable, only environment scrubbing is applied (graceful degradation).
func applyBashSandbox(cmd *exec.Cmd, workDir string) error {
	// Scrub sensitive environment variables from the subprocess
	ScrubEnvironment(cmd)

	// Attempt Landlock filesystem restriction
	if err := applyLandlock(cmd, workDir); err != nil {
		slog.Debug("landlock not available, using env-scrub-only sandbox",
			"error", err)
	}

	return nil
}

// landlockAccessFs represents Landlock filesystem access rights.
const (
	landlockAccessFsExec  = 0x1 // LANDLOCK_ACCESS_FS_EXECUTE
	landlockAccessFsWrite = 0x2 // LANDLOCK_ACCESS_FS_WRITE_FILE
	landlockAccessFsRead  = 0x4 // LANDLOCK_ACCESS_FS_READ_FILE
)

// landlockRuleTypePathBeneath is the Landlock rule type for path-based rules.
const landlockRuleTypePathBeneath = 1

// landlockCreateRuleset creates a Landlock ruleset with the given access rights mask.
// Returns the ruleset file descriptor, or an error if Landlock is not supported.
func landlockCreateRuleset(accessRights uint64) (int, error) {
	attr := unix.LandlockRulesetAttr{
		Access_fs: accessRights,
	}
	fd, _, errno := syscall.Syscall6(
		unix.SYS_LANDLOCK_CREATE_RULESET,
		uintptr(unsafe.Pointer(&attr)),
		uintptr(unsafe.Sizeof(attr)),
		0, // flags
		0, 0, 0,
	)
	if int32(errno) < 0 {
		return -1, errno
	}
	return int(fd), nil
}

// landlockAddRule adds a path-beneath rule to a Landlock ruleset.
func landlockAddRule(rulesetFd int, allowedAccess uint64, parentFd int, allowSymlinks bool) error {
	var flags uint64
	if allowSymlinks {
		flags = 0x1 // LANDLOCK_ACCESS_FS_IOCTL_DEV — actually LANDLOCK_ACCESS_FS_IOCTL
		// Note: symlink allowance is controlled via flags in Landlock ABI v2+
	}
	attr := unix.LandlockPathBeneathAttr{
		Allowed_access: allowedAccess,
		Parent_fd:      int32(parentFd),
	}
	_, _, errno := syscall.Syscall6(
		unix.SYS_LANDLOCK_ADD_RULE,
		uintptr(rulesetFd),
		uintptr(landlockRuleTypePathBeneath),
		uintptr(unsafe.Pointer(&attr)),
		uintptr(flags),
		0, 0,
	)
	if int32(errno) < 0 {
		return errno
	}
	return nil
}

// landlockRestrictSelf applies a Landlock ruleset to the current process.
func landlockRestrictSelf(rulesetFd int, flags uint64) error {
	_, _, errno := syscall.Syscall(
		unix.SYS_LANDLOCK_RESTRICT_SELF,
		uintptr(rulesetFd),
		uintptr(flags),
		0,
	)
	if int32(errno) < 0 {
		return errno
	}
	return nil
}

// applyLandlock attempts to set up Landlock filesystem restrictions.
// This applies to the current process and is inherited by child processes.
// It restricts filesystem access to the work directory, /tmp, and essential
// system paths needed for subprocess execution.
func applyLandlock(cmd *exec.Cmd, workDir string) error {
	// Define allowed access: read + write + execute for filesystem
	allowedAccess := uint64(landlockAccessFsExec | landlockAccessFsWrite | landlockAccessFsRead)

	// Create the ruleset
	rulesetFd, err := landlockCreateRuleset(allowedAccess)
	if err != nil {
		return fmt.Errorf("create landlock ruleset: %w", err)
	}
	defer func() { _ = syscall.Close(rulesetFd) }()

	// Helper to open a path for Landlock rules
	openForLandlock := func(path string) (int, error) {
		fd, err := syscall.Open(path, syscall.O_RDONLY|syscall.O_CLOEXEC, 0)
		if err != nil {
			return -1, err
		}
		return fd, nil
	}

	// Allow access to the work directory
	if workDir != "" {
		if fd, err := openForLandlock(workDir); err == nil {
			_ = landlockAddRule(rulesetFd, allowedAccess, fd, false)
			_ = syscall.Close(fd)
		}
	}

	// Allow access to /tmp
	if fd, err := openForLandlock("/tmp"); err == nil {
		_ = landlockAddRule(rulesetFd, allowedAccess, fd, false)
		_ = syscall.Close(fd)
	}

	// Allow access to /dev/null (needed for stdin redirect)
	if fd, err := openForLandlock("/dev"); err == nil {
		_ = landlockAddRule(rulesetFd, allowedAccess, fd, false)
		_ = syscall.Close(fd)
	}

	// Allow read access to /usr (for bash, env, etc.)
	if fd, err := openForLandlock("/usr"); err == nil {
		_ = landlockAddRule(rulesetFd, landlockAccessFsExec|landlockAccessFsRead, fd, false)
		_ = syscall.Close(fd)
	}

	// Allow read access to /bin (for basic binaries)
	if fd, err := openForLandlock("/bin"); err == nil {
		_ = landlockAddRule(rulesetFd, landlockAccessFsExec|landlockAccessFsRead, fd, false)
		_ = syscall.Close(fd)
	}

	// Allow read access to /lib and /lib64 (for shared libraries)
	for _, libDir := range []string{"/lib", "/lib64"} {
		if fd, err := openForLandlock(libDir); err == nil {
			_ = landlockAddRule(rulesetFd, landlockAccessFsExec|landlockAccessFsRead, fd, false)
			_ = syscall.Close(fd)
		}
	}

	// Apply Landlock to the current process (inherited by children)
	if err := landlockRestrictSelf(rulesetFd, 0); err != nil {
		return fmt.Errorf("apply landlock restrictions: %w", err)
	}

	return nil
}

// ScrubEnvironment removes sensitive environment variables from the subprocess.
func ScrubEnvironment(cmd *exec.Cmd) {
	// Start from the current environment
	env := os.Environ()

	// Sensitive env vars to remove (API keys, tokens, secrets)
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

	// Build a set of sensitive var prefixes to check
	sensitivePrefixes := []string{
		"API_KEY",
		"TOKEN",
		"SECRET",
		"PASSWORD",
		"CREDENTIAL",
		"AUTH",
	}

	// Filter out sensitive variables
	scrubbed := make([]string, 0, len(env))
	for _, e := range env {
		key := e
		if idx := indexOf(e, '='); idx >= 0 {
			key = e[:idx]
		}

		// Check exact matches
		sensitive := false
		for _, sv := range sensitiveVars {
			if key == sv {
				sensitive = true
				break
			}
		}

		// Check prefix matches (case-insensitive for safety)
		if !sensitive {
			for _, prefix := range sensitivePrefixes {
				if hasPrefixFold(key, prefix) {
					sensitive = true
					break
				}
			}
		}

		if !sensitive {
			scrubbed = append(scrubbed, e)
		}
	}

	// Re-add non-interactive environment variables
	scrubbed = append(scrubbed,
		"CI=true",
		"DEBIAN_FRONTEND=noninteractive",
		"npm_config_yes=true",
		"PIP_NO_INPUT=1",
		"YARN_ENABLE_IMMUTABLE_INSTALLS=false",
	)

	cmd.Env = scrubbed
}

// indexOf returns the index of the first occurrence of c in s, or -1.
func indexOf(s string, c byte) int {
	for i := 0; i < len(s); i++ {
		if s[i] == c {
			return i
		}
	}
	return -1
}

// hasPrefixFold checks if s starts with prefix (case-insensitive).
func hasPrefixFold(s, prefix string) bool {
	if len(s) < len(prefix) {
		return false
	}
	for i := 0; i < len(prefix); i++ {
		a := s[i]
		b := prefix[i]
		if a >= 'A' && a <= 'Z' {
			a += 32
		}
		if b >= 'A' && b <= 'Z' {
			b += 32
		}
		if a != b {
			return false
		}
	}
	return true
}