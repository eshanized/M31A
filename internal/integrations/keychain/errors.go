package keychain

import "errors"

var (
	// ErrKeychainUnavailable is returned when the OS keychain is not available
	// (e.g., D-Bus not running on Linux, security CLI missing on macOS).
	ErrKeychainUnavailable = errors.New("keychain unavailable")

	// ErrKeyNotFound is returned when a requested key does not exist in the keychain.
	ErrKeyNotFound = errors.New("key not found")

	// ErrKeychainDecrypt is returned when the keychain secret blob is corrupted
	// or unreadable (e.g., GPG decryption failure on Linux pass backend).
	ErrKeychainDecrypt = errors.New("keychain secret blob is corrupted or unreadable")

	// ErrNotImplemented is returned on platforms where keychain operations
	// are not supported.
	ErrNotImplemented = errors.New("not implemented on this platform")
)
