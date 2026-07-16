package keychain

import "sync/atomic"

const (
	servicePrefix = "m31a/"
	// AccountName is the keychain account identifier used across all platforms.
	AccountName = "m31a"
)

// Keychain provides OS-native secure storage for API keys.
// Each platform implements this interface using the native secret storage mechanism.
type Keychain interface {
	// Get retrieves the value for the given service name.
	// Returns ErrKeyNotFound if the key does not exist.
	// Returns ErrKeychainUnavailable if the keychain backend is unavailable.
	Get(service string) (string, error)

	// Set stores a value for the given service name.
	// If a value already exists for this service, it is overwritten.
	// Returns ErrKeychainUnavailable if the keychain backend is unavailable.
	Set(service, value string) error

	// Delete removes the value for the given service name.
	// Returns ErrKeyNotFound if the key does not exist.
	// Returns ErrKeychainUnavailable if the keychain backend is unavailable.
	Delete(service string) error
}

// New returns a platform-specific Keychain implementation.
// On linux: returns a linuxKeychain backed by D-Bus Secret Service with pass CLI fallback.
// On darwin: returns a macOSKeychain backed by /usr/bin/security CLI.
// On windows: returns a windowsKeychain backed by Windows Credential Manager.
// Each platform file (keychain_linux.go, keychain_darwin.go, keychain_windows.go)
// provides its own New() implementation via build tags.

// cachedKeychain wraps a Keychain and caches availability.
// Once any operation returns ErrKeychainUnavailable, all subsequent operations
// return ErrKeychainUnavailable immediately without attempting the backend.
// This prevents repeated D-Bus/pass connection attempts and suppresses
// duplicate warning logs.
type cachedKeychain struct {
	inner       Keychain
	unavailable atomic.Bool
}

// NewCached wraps an existing Keychain with availability caching.
// If the inner keychain is nil, returns nil.
func NewCached(inner Keychain) Keychain {
	if inner == nil {
		return nil
	}
	return &cachedKeychain{inner: inner}
}

func (c *cachedKeychain) Get(service string) (string, error) {
	if c.unavailable.Load() {
		return "", ErrKeychainUnavailable
	}
	val, err := c.inner.Get(service)
	if err != nil && (err == ErrKeychainUnavailable || err == ErrNotImplemented) {
		c.unavailable.Store(true)
	}
	return val, err
}

func (c *cachedKeychain) Set(service, value string) error {
	if c.unavailable.Load() {
		return ErrKeychainUnavailable
	}
	err := c.inner.Set(service, value)
	if err != nil && (err == ErrKeychainUnavailable || err == ErrNotImplemented) {
		c.unavailable.Store(true)
	}
	return err
}

func (c *cachedKeychain) Delete(service string) error {
	if c.unavailable.Load() {
		return ErrKeychainUnavailable
	}
	err := c.inner.Delete(service)
	if err != nil && (err == ErrKeychainUnavailable || err == ErrNotImplemented) {
		c.unavailable.Store(true)
	}
	return err
}
