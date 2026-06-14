//go:build linux

package keychain

import (
	"fmt"
	"os/exec"
	"regexp"
	"strings"

	"github.com/godbus/dbus/v5"
)

const (
	secretServiceName = "org.freedesktop.secrets"
	secretServicePath = "/org/freedesktop/secrets"
)

// validServiceName checks that the service parameter contains only lowercase
// ASCII letters (a-z) after the m31a/ prefix. This prevents command injection
// when passing the service name to the pass CLI.
var validServiceName = regexp.MustCompile(`^[a-z0-9-]+$`)

type linuxKeychain struct{}

// New returns a Linux keychain backed by D-Bus Secret Service with pass CLI fallback.
func New() (Keychain, error) {
	return &linuxKeychain{}, nil
}

func validateService(service string) error {
	if !validServiceName.MatchString(service) {
		return ErrKeychainUnavailable
	}
	return nil
}

// Get retrieves the secret for the given service.
// Attempts D-Bus Secret Service first, then falls back to pass CLI.
func (k *linuxKeychain) Get(service string) (string, error) {
	if err := validateService(service); err != nil {
		return "", err
	}

	val, err := k.dbusGet(service)
	if err == nil {
		return val, nil
	}
	if !isDBusUnavailable(err) {
		return "", err
	}

	return k.passGet(service)
}

func (k *linuxKeychain) dbusGet(service string) (string, error) {
	conn, err := dbus.ConnectSessionBus()
	if err != nil {
		return "", err
	}
	defer conn.Close() //nolint:errcheck

	obj := conn.Object(secretServiceName, secretServicePath)
	servicePath := servicePrefix + service

	// Open a session for the GetSecret call
	var sessionPath dbus.ObjectPath
	sessionCall := obj.Call("org.freedesktop.Secret.Service.OpenSession", 0, "plain", dbus.MakeVariant(""))
	if sessionCall.Err == nil {
		// Store ignores the first output (algorithm output); we only need the session path.
		var ignore dbus.Variant
		_ = sessionCall.Store(&ignore, &sessionPath)
	}
	defer func() {
		if sessionPath != "" {
			conn.Object(secretServiceName, sessionPath).Call("org.freedesktop.Secret.Session.Close", 0)
		}
	}()

	// SearchItems returns two arrays: unlocked items, locked items
	var unlocked, locked []dbus.ObjectPath
	call := obj.Call("org.freedesktop.Secret.Service.SearchItems", 0, map[string]string{
		"service": servicePath,
	})
	if call.Err != nil {
		return "", call.Err
	}
	if err := call.Store(&unlocked, &locked); err != nil {
		return "", err
	}

	// Prefer unlocked items; fall back to locked
	items := append(unlocked, locked...)
	if len(items) == 0 {
		return "", ErrKeyNotFound
	}

	// Get the secret from the first matching item
	itemObj := conn.Object(secretServiceName, items[0])
	var secret struct {
		Session     dbus.ObjectPath
		Parameters  []byte
		Value       []byte
		ContentType string
	}
	call = itemObj.Call("org.freedesktop.Secret.Item.GetSecret", 0, sessionPath)
	if call.Err != nil {
		return "", call.Err
	}
	if err := call.Store(&secret); err != nil {
		return "", err
	}

	return string(secret.Value), nil
}

func (k *linuxKeychain) passGet(service string) (string, error) {
	out, err := exec.Command("pass", "show", servicePrefix+service).Output()
	if err != nil {
		if isPassNotFound(err) {
			return "", ErrKeyNotFound
		}
		if isPassGPGFailure(err) {
			return "", fmt.Errorf("GPG decryption failed — try 'pass init <gpg-id>' or re-store your API key with /settings: %w", ErrKeychainDecrypt)
		}
		if isPassUnavailable(err) {
			return "", ErrKeychainUnavailable
		}
		return "", err
	}
	return strings.TrimSpace(string(out)), nil
}

// Set stores a secret for the given service.
// Attempts D-Bus Secret Service first, then falls back to pass CLI.
func (k *linuxKeychain) Set(service, value string) error {
	if err := validateService(service); err != nil {
		return err
	}

	err := k.dbusSet(service, value)
	if err == nil {
		return nil
	}
	if !isDBusUnavailable(err) {
		return err
	}

	return k.passSet(service, value)
}

func (k *linuxKeychain) dbusSet(service, value string) error {
	conn, err := dbus.ConnectSessionBus()
	if err != nil {
		return err
	}
	defer conn.Close() //nolint:errcheck

	obj := conn.Object(secretServiceName, secretServicePath)
	servicePath := servicePrefix + service
	label := "M31A API Key: " + service

	// Open a plain-text session — this is required before creating/updating items.
	var sessionPath dbus.ObjectPath
	sessionCall := obj.Call("org.freedesktop.Secret.Service.OpenSession", 0, "plain", dbus.MakeVariant(""))
	if sessionCall.Err != nil {
		return sessionCall.Err
	}
	// Store ignores the first output (algorithm output); we only need the session path.
	var ignore dbus.Variant
	if err := sessionCall.Store(&ignore, &sessionPath); err != nil {
		return fmt.Errorf("open session: %w", err)
	}
	defer conn.Object(secretServiceName, sessionPath).Call("org.freedesktop.Secret.Session.Close", 0)

	// Try to find existing item first
	var unlocked, locked []dbus.ObjectPath
	call := obj.Call("org.freedesktop.Secret.Service.SearchItems", 0, map[string]string{
		"service": servicePath,
	})
	if call.Err == nil {
		_ = call.Store(&unlocked, &locked)
	}
	items := append(unlocked, locked...)

	if len(items) > 0 {
		// Update existing item using the opened session
		itemObj := conn.Object(secretServiceName, items[0])
		newSecret := fmtSecret(sessionPath, value)
		call = itemObj.Call("org.freedesktop.Secret.Item.SetSecret", 0, newSecret)
		return call.Err
	}

	// Create a new item in the default collection
	collection := dbus.ObjectPath("/org/freedesktop/secrets/aliases/default")
	secret := fmtSecret(sessionPath, value)
	props := map[string]any{
		"org.freedesktop.Secret.Item.Label": label,
		"org.freedesktop.Secret.Item.Attributes": map[string]string{
			"service": servicePath,
			"account": AccountName,
		},
	}
	call = obj.Call("org.freedesktop.Secret.Service.CreateItem", 0, collection, props, secret, true)
	return call.Err
}

func (k *linuxKeychain) passSet(service, value string) error {
	cmd := exec.Command("pass", "insert", "-U", "-f", servicePrefix+service)
	cmd.Stdin = strings.NewReader(value)
	err := cmd.Run()
	if err != nil {
		if isPassUnavailable(err) {
			return ErrKeychainUnavailable
		}
		return err
	}
	return nil
}

// Delete removes the secret for the given service.
// Attempts D-Bus Secret Service first, then falls back to pass CLI.
func (k *linuxKeychain) Delete(service string) error {
	if err := validateService(service); err != nil {
		return err
	}

	err := k.dbusDelete(service)
	if err == nil {
		return nil
	}
	if !isDBusUnavailable(err) {
		return err
	}

	return k.passDelete(service)
}

func (k *linuxKeychain) dbusDelete(service string) error {
	conn, err := dbus.ConnectSessionBus()
	if err != nil {
		return err
	}
	defer conn.Close() //nolint:errcheck

	obj := conn.Object(secretServiceName, secretServicePath)
	servicePath := servicePrefix + service

	var items []dbus.ObjectPath
	call := obj.Call("org.freedesktop.Secret.Service.SearchItems", 0, map[string]string{
		"service": servicePath,
	})
	if call.Err != nil {
		return call.Err
	}
	if err := call.Store(&items); err != nil {
		return err
	}
	if len(items) == 0 {
		return ErrKeyNotFound
	}

	// Delete each matching item
	for _, item := range items {
		itemObj := conn.Object(secretServiceName, item)
		call = itemObj.Call("org.freedesktop.Secret.Item.Delete", 0)
		if call.Err != nil {
			return call.Err
		}
	}
	return nil
}

func (k *linuxKeychain) passDelete(service string) error {
	err := exec.Command("pass", "rm", "-f", servicePrefix+service).Run()
	if err != nil {
		if isPassNotFound(err) {
			return ErrKeyNotFound
		}
		if isPassUnavailable(err) {
			return ErrKeychainUnavailable
		}
		return err
	}
	return nil
}

// Helper: construct a Secret struct for D-Bus using the provided session path.
// sessionPath must be the path returned by OpenSession — never pass an empty path.
func fmtSecret(sessionPath dbus.ObjectPath, value string) map[string]any {
	return map[string]any{
		"session":      sessionPath,
		"parameters":   []byte{},
		"value":        []byte(value),
		"content_type": "text/plain",
	}
}

// isDBusUnavailable returns true if the error indicates D-Bus is not available
// (session bus not running, no keyring daemon, etc.).
func isDBusUnavailable(err error) bool {
	if err == nil {
		return false
	}
	// dbus.ErrClosed is returned when the session bus is not running
	if err == dbus.ErrClosed {
		return true
	}
	// Also catch "connection refused" style errors
	errStr := err.Error()
	return strings.Contains(errStr, "connection refused") ||
		strings.Contains(errStr, "no such file or directory") ||
		strings.Contains(errStr, "dbus")
}

// isPassUnavailable returns true if the pass CLI is not installed or not available.
func isPassUnavailable(err error) bool {
	if err == nil {
		return false
	}
	if exitErr, ok := err.(*exec.ExitError); ok {
		stderr := string(exitErr.Stderr)
		// Only treat as "unavailable" if stderr indicates pass isn't installed
		// or the password store isn't initialized. Other errors (GPG failures,
		// pinentry timeouts) mean pass IS available but the operation failed.
		return strings.Contains(stderr, "not found") ||
			strings.Contains(stderr, "not installed") ||
			strings.Contains(stderr, "No password store") ||
			strings.Contains(stderr, "password-store is empty")
	}
	// exec.Error means the binary wasn't found
	_, isExecErr := err.(*exec.Error)
	return isExecErr
}

// isPassNotFound returns true if pass reports the password was not found.
func isPassNotFound(err error) bool {
	if exitErr, ok := err.(*exec.ExitError); ok {
		stderr := string(exitErr.Stderr)
		return strings.Contains(stderr, "not found") || strings.Contains(stderr, "not in")
	}
	return false
}

// isPassGPGFailure returns true if pass failed due to a GPG decryption error
// (e.g., bad passphrase, corrupted secret blob).
func isPassGPGFailure(err error) bool {
	exitErr, ok := err.(*exec.ExitError)
	if !ok {
		return false
	}
	stderr := string(exitErr.Stderr)
	return strings.Contains(stderr, "gpg: decryption failed") ||
		strings.Contains(stderr, "bad passphrase") ||
		strings.Contains(stderr, "gpg: error")
}
