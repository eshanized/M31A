package mocks

import (
	"errors"

	"github.com/eshanized/M31A/internal/keychain"
)

// MockKeychain implements keychain.Keychain with an in-memory map for testing.
type MockKeychain struct {
	Store map[string]string
}

// Compile-time interface check.
var _ keychain.Keychain = (*MockKeychain)(nil)

// NewMockKeychain creates a MockKeychain with an empty store.
func NewMockKeychain() *MockKeychain {
	return &MockKeychain{Store: make(map[string]string)}
}

func (m *MockKeychain) Get(service string) (string, error) {
	val, ok := m.Store[service]
	if !ok {
		return "", errors.New("not found")
	}
	return val, nil
}

func (m *MockKeychain) Set(service, value string) error {
	m.Store[service] = value
	return nil
}

func (m *MockKeychain) Delete(service string) error {
	delete(m.Store, service)
	return nil
}
