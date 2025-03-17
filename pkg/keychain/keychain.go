package keychain

const servicePrefix = "m31a/"

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

// newFunc is set by platform-specific init() functions in build-tag-guarded files.
// Each platform file (keychain_linux.go, keychain_darwin.go, keychain_windows.go)
// assigns its own constructor to this variable.
var newFunc func() (Keychain, error)

// New returns a platform-specific Keychain implementation.
// On linux: returns a linuxKeychain backed by D-Bus Secret Service with pass CLI fallback.
// On darwin: returns a macOSKeychain backed by /usr/bin/security CLI.
// On windows: returns a windowsKeychain stub that returns ErrNotImplemented.
func New() (Keychain, error) {
	if newFunc == nil {
		return nil, ErrKeychainUnavailable
	}
	return newFunc()
}
