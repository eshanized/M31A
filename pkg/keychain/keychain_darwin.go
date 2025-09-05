//go:build darwin

package keychain

import (
	"os/exec"
	"regexp"
	"strings"
)

// accountName is the keychain account identifier (from shared constant).
const accountName = AccountName

// validServiceName checks that the service parameter contains only lowercase
// ASCII letters (a-z) after the m31a/ prefix. This prevents command injection
// when passing the service name to the security CLI.
var validServiceName = regexp.MustCompile(`^[a-z]+$`)

type macOSKeychain struct{}

func init() {
	newFunc = newmacOSKeychain
}

func newmacOSKeychain() (Keychain, error) {
	return &macOSKeychain{}, nil
}

func validateService(service string) error {
	if !validServiceName.MatchString(service) {
		return ErrNotImplemented
	}
	return nil
}

// Get retrieves the password from the macOS keychain using the /usr/bin/security CLI.
func (k *macOSKeychain) Get(service string) (string, error) {
	if err := validateService(service); err != nil {
		return "", err
	}

	out, err := exec.Command(
		"/usr/bin/security",
		"find-generic-password",
		"-s", servicePrefix+service,
		"-wa", accountName,
	).Output()
	if err != nil {
		if strings.Contains(string(out), "could not be found") {
			return "", ErrKeyNotFound
		}
		return "", err
	}
	return strings.TrimSpace(string(out)), nil
}

// Set stores a password in the macOS keychain using the /usr/bin/security CLI.
// Uses -U flag to update the item if it already exists.
func (k *macOSKeychain) Set(service, value string) error {
	if err := validateService(service); err != nil {
		return err
	}

	cmd := exec.Command(
		"/usr/bin/security",
		"add-generic-password",
		"-U", // update if exists
		"-s", servicePrefix+service,
		"-a", accountName,
		"-w", value,
	)
	return cmd.Run()
}

// Delete removes a password from the macOS keychain using the /usr/bin/security CLI.
func (k *macOSKeychain) Delete(service string) error {
	if err := validateService(service); err != nil {
		return err
	}

	out, err := exec.Command(
		"/usr/bin/security",
		"delete-generic-password",
		"-s", servicePrefix+service,
		"-a", accountName,
	).CombinedOutput()
	if strings.Contains(string(out), "could not be found") {
		return ErrKeyNotFound
	}
	return err
}
