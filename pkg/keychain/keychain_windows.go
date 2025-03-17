//go:build windows

package keychain

type windowsKeychain struct{}

func init() {
	newFunc = newWindowsKeychain
}

func newWindowsKeychain() (Keychain, error) {
	return &windowsKeychain{}, nil
}

// Get is not implemented on Windows (V1 stub).
func (k *windowsKeychain) Get(service string) (string, error) {
	return "", ErrNotImplemented
}

// Set is not implemented on Windows (V1 stub).
func (k *windowsKeychain) Set(service, value string) error {
	return ErrNotImplemented
}

// Delete is not implemented on Windows (V1 stub).
func (k *windowsKeychain) Delete(service string) error {
	return ErrNotImplemented
}
