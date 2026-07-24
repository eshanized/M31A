package keychain

import (
	"sync/atomic"
	"time"
)

const (
	servicePrefix = "m31a/"
	// AccountName is the keychain account identifier used across all platforms.
	AccountName = "m31a"

	// blacklistTTL is how long the keychain stays blacklisted after a transient failure.
	// After this period, the cached keychain will retry the backend.
	blacklistTTL = 5 * time.Minute
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
// Once any operation returns ErrKeychainUnavailable, the keychain is blacklisted
// for blacklistTTL duration. After the TTL expires, the backend is retried.
// This prevents repeated D-Bus/pass connection attempts while allowing recovery.
type cachedKeychain struct {
	inner           Keychain
	unavailableSince atomic.Int64 // Unix nanoseconds when blacklisted; 0 = available
}

// isBlacklisted reports whether the keychain is currently blacklisted.
// Returns false if the TTL has expired (allowing retry).
func (c *cachedKeychain) isBlacklisted() bool {
	ts := c.unavailableSince.Load()
	if ts == 0 {
		return false
	}
	return time.Since(time.Unix(0, ts)) < blacklistTTL
}

// blacklist marks the keychain as unavailable with current timestamp.
func (c *cachedKeychain) blacklist() {
	c.unavailableSince.Store(time.Now().UnixNano())
}

// clearBlacklist resets availability so next call retries the backend.
func (c *cachedKeychain) clearBlacklist() {
	c.unavailableSince.Store(0)
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
	if c.isBlacklisted() {
		return "", ErrKeychainUnavailable
	}
	val, err := c.inner.Get(service)
	if err != nil && (err == ErrKeychainUnavailable || err == ErrNotImplemented) {
		c.blacklist()
		return val, err
	}
	// Success clears any stale blacklist
	c.clearBlacklist()
	return val, err
}

func (c *cachedKeychain) Set(service, value string) error {
	if c.isBlacklisted() {
		return ErrKeychainUnavailable
	}
	err := c.inner.Set(service, value)
	if err != nil && (err == ErrKeychainUnavailable || err == ErrNotImplemented) {
		c.blacklist()
		return err
	}
	c.clearBlacklist()
	return err
}

func (c *cachedKeychain) Delete(service string) error {
	if c.isBlacklisted() {
		return ErrKeychainUnavailable
	}
	err := c.inner.Delete(service)
	if err != nil && (err == ErrKeychainUnavailable || err == ErrNotImplemented) {
		c.blacklist()
		return err
	}
	c.clearBlacklist()
	return err
}
