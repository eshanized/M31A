package keychain

import "errors"

var (
	// ErrKeychainUnavailable is returned when the OS keychain is not available
	// (e.g., D-Bus not running on Linux, security CLI missing on macOS).
	ErrKeychainUnavailable = errors.New("keychain unavailable")

	// ErrKeyNotFound is returned when a requested key does not exist in the keychain.
	ErrKeyNotFound = errors.New("key not found")

	// ErrNotImplemented is returned on platforms where keychain operations
	// are not supported (Windows V1 stub).
	ErrNotImplemented = errors.New("not implemented on this platform")
)
