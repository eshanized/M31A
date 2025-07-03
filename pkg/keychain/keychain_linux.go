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
var validServiceName = regexp.MustCompile(`^[a-z]+$`)

type linuxKeychain struct{}

func init() {
	newFunc = newLinuxKeychain
}

func newLinuxKeychain() (Keychain, error) {
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
	defer conn.Close()

	obj := conn.Object(secretServiceName, secretServicePath)
	servicePath := servicePrefix + service

	// SearchItems returns the object paths of matching items
	var items []dbus.ObjectPath
	call := obj.Call("org.freedesktop.Secret.Service.SearchItems", 0, map[string]string{
		"service": servicePath,
	})
	if call.Err != nil {
		return "", call.Err
	}
	if err := call.Store(&items); err != nil {
		return "", err
	}
	if len(items) == 0 {
		return "", ErrKeyNotFound
	}

	// Get the secret from the first matching item
	itemObj := conn.Object(secretServiceName, items[0])
	var secret struct {
		Session     dbus.ObjectPath
		Value       []byte
		ContentType string
	}
	call = itemObj.Call("org.freedesktop.Secret.Item.GetSecret", 0, dbus.MakeVariant(""))
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
			return "", fmt.Errorf("gpg decrypt: %w", ErrKeychainDecrypt)
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
	defer conn.Close()

	obj := conn.Object(secretServiceName, secretServicePath)
	servicePath := servicePrefix + service
	label := "M31A API Key: " + service

	// Try to find existing item first
	var items []dbus.ObjectPath
	call := obj.Call("org.freedesktop.Secret.Service.SearchItems", 0, map[string]string{
		"service": servicePath,
	})
	if call.Err == nil {
		call.Store(&items)
	}

	if len(items) > 0 {
		// Update existing item
		itemObj := conn.Object(secretServiceName, items[0])
		newSecret := fmtSecret(conn, value)
		call = itemObj.Call("org.freedesktop.Secret.Item.SetSecret", 0, newSecret)
		return call.Err
	}

	// Create a new item in the default collection
	collection := dbus.ObjectPath("/org/freedesktop/secrets/aliases/default")
	var sessionPath dbus.ObjectPath
	call = obj.Call("org.freedesktop.Secret.Service.OpenSession", 0, "plain", dbus.MakeVariant(""))
	if call.Err != nil {
		return call.Err
	}
	call.Store(&sessionPath, nil)

	secret := fmtSecret(conn, value)
	props := map[string]interface{}{
		"org.freedesktop.Secret.Item.Label": label,
		"org.freedesktop.Secret.Item.Attributes": map[string]string{
			"service": servicePath,
			"account": "m31a",
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
	defer conn.Close()

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

// Helper: construct a Secret struct for D-Bus
func fmtSecret(conn *dbus.Conn, value string) map[string]interface{} {
	var sessionPath dbus.ObjectPath // empty = first session
	return map[string]interface{}{
		"session":     sessionPath,
		"value":       []byte(value),
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

// isPassUnavailable returns true if the pass CLI is not installed.
func isPassUnavailable(err error) bool {
	if err == nil {
		return false
	}
	if exitErr, ok := err.(*exec.ExitError); ok {
		// pass exits with 1 for various errors; check stderr for "not found"
		return len(exitErr.Stderr) > 0
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
