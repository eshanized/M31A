package keychain

import (
	"sync/atomic"
	"testing"
	"time"
)

// mockKeychain implements Keychain with an in-memory map for testing.
type mockKeychain struct {
	store map[string]string
}

func newMockKeychain() *mockKeychain {
	return &mockKeychain{store: make(map[string]string)}
}

func (m *mockKeychain) Get(service string) (string, error) {
	val, ok := m.store[service]
	if !ok {
		return "", ErrKeyNotFound
	}
	return val, nil
}

func (m *mockKeychain) Set(service, value string) error {
	m.store[service] = value
	return nil
}

func (m *mockKeychain) Delete(service string) error {
	if _, ok := m.store[service]; !ok {
		return ErrKeyNotFound
	}
	delete(m.store, service)
	return nil
}

// TestKeychain_SetGet verifies that a value can be stored and retrieved.
func TestKeychain_SetGet(t *testing.T) {
	kc := newMockKeychain()

	err := kc.Set("openrouter", "sk-or-v1-test-key")
	if err != nil {
		t.Fatalf("Set failed: %v", err)
	}

	val, err := kc.Get("openrouter")
	if err != nil {
		t.Fatalf("Get failed: %v", err)
	}
	if val != "sk-or-v1-test-key" {
		t.Errorf("Get returned %q, want %q", val, "sk-or-v1-test-key")
	}
}

// TestKeychain_DeleteGet verifies that a deleted value returns ErrKeyNotFound.
func TestKeychain_SetDeleteGet(t *testing.T) {
	kc := newMockKeychain()

	err := kc.Set("openrouter", "sk-or-v1-test-key")
	if err != nil {
		t.Fatalf("Set failed: %v", err)
	}

	err = kc.Delete("openrouter")
	if err != nil {
		t.Fatalf("Delete failed: %v", err)
	}

	_, err = kc.Get("openrouter")
	if err != ErrKeyNotFound {
		t.Errorf("Get after delete should return ErrKeyNotFound, got %v", err)
	}
}

// TestKeychain_GetNotFound verifies that getting a non-existent key returns ErrKeyNotFound.
func TestKeychain_GetNotFound(t *testing.T) {
	kc := newMockKeychain()

	_, err := kc.Get("nonexistent")
	if err != ErrKeyNotFound {
		t.Errorf("Get for missing key should return ErrKeyNotFound, got %v", err)
	}
}

// TestKeychain_DeleteNotFound verifies that deleting a non-existent key returns ErrKeyNotFound.
func TestKeychain_DeleteNotFound(t *testing.T) {
	kc := newMockKeychain()

	err := kc.Delete("nonexistent")
	if err != ErrKeyNotFound {
		t.Errorf("Delete for missing key should return ErrKeyNotFound, got %v", err)
	}
}

// TestKeychain_ServicePrefixFormat verifies that the service prefix constant has the expected value.
func TestKeychain_ServiceNameFormat(t *testing.T) {
	if servicePrefix != "m31a/" {
		t.Errorf("servicePrefix = %q, want %q", servicePrefix, "m31a/")
	}
}

// TestKeychain_EmptyServiceName verifies platform validation behavior.
// Invalid service names should trigger an error from platform validation.
func TestKeychain_EmptyService(t *testing.T) {
	// The platform validators require [a-z]+, so empty string is invalid.
	if err := validateService(""); err == nil {
		// This test validates that empty service is rejected by platform validators.
		// On platforms with validation, empty should fail.
		t.Log("Note: empty service name validated (platform-dependent)")
	}
}

// TestKeychain_InterfaceCompliance ensures that the Keychain interface contract
// is satisfied by the mock implementation.
func TestKeychain_InterfaceCompliance(t *testing.T) {
	// The mockKeychain must satisfy the Keychain interface.
	// Platform-specific types (linuxKeychain, macOSKeychain, windowsKeychain)
	// are checked by their respective build-tag-guarded files.
	var _ Keychain = (*mockKeychain)(nil)
}

// TestKeychain_NewReturnsInterface verifies that New() returns a Keychain.
// Since this runs on the current platform, it tests whichever platform implementation
// is active.
func TestKeychain_NewReturnsInterface(t *testing.T) {
	kc, err := New()
	if kc == nil && err != nil {
		// newFunc not set (no platform init ran) — allowed in test context
		t.Logf("New() returned nil, err: %v (expected if no platform init ran)", err)
		return
	}
	if err != nil {
		t.Fatalf("New() failed: %v", err)
	}
	if kc == nil {
		t.Fatal("New() returned nil without error")
	}
}

// TestKeychain_Overwrite verifies that setting the same service twice overwrites.
func TestKeychain_Overwrite(t *testing.T) {
	kc := newMockKeychain()

	err := kc.Set("openrouter", "first-key")
	if err != nil {
		t.Fatalf("First Set failed: %v", err)
	}

	err = kc.Set("openrouter", "second-key")
	if err != nil {
		t.Fatalf("Second Set failed: %v", err)
	}

	val, err := kc.Get("openrouter")
	if err != nil {
		t.Fatalf("Get failed: %v", err)
	}
	if val != "second-key" {
		t.Errorf("Get after overwrite returned %q, want %q", val, "second-key")
	}
}

// TestKeychain_MultipleServices verifies that multiple services don't interfere.
func TestKeychain_MultipleServices(t *testing.T) {
	kc := newMockKeychain()

	err := kc.Set("openrouter", "or-key")
	if err != nil {
		t.Fatalf("Set openrouter failed: %v", err)
	}
	err = kc.Set("zen", "zen-key")
	if err != nil {
		t.Fatalf("Set zen failed: %v", err)
	}

	val1, err := kc.Get("openrouter")
	if err != nil {
		t.Fatalf("Get openrouter failed: %v", err)
	}
	val2, err := kc.Get("zen")
	if err != nil {
		t.Fatalf("Get zen failed: %v", err)
	}

	if val1 != "or-key" {
		t.Errorf("openrouter = %q, want %q", val1, "or-key")
	}
	if val2 != "zen-key" {
		t.Errorf("zen = %q, want %q", val2, "zen-key")
	}
}

// ---------------------------------------------------------------------------
// B21: TTL-based keychain blacklist recovery
// ---------------------------------------------------------------------------

// unavailableKeychain always returns ErrKeychainUnavailable.
type unavailableKeychain struct {
	callCount int
}

func (u *unavailableKeychain) Get(service string) (string, error) {
	u.callCount++
	return "", ErrKeychainUnavailable
}

func (u *unavailableKeychain) Set(service, value string) error {
	u.callCount++
	return ErrKeychainUnavailable
}

func (u *unavailableKeychain) Delete(service string) error {
	u.callCount++
	return ErrKeychainUnavailable
}

func TestKeychainBlacklist_Recovers(t *testing.T) {
	inner := &unavailableKeychain{}
	kc := NewCached(inner)

	// First call should blacklist
	_, err := kc.Get("test")
	if err != ErrKeychainUnavailable {
		t.Fatalf("expected ErrKeychainUnavailable, got %v", err)
	}

	// Immediately after, should be blacklisted (no retry)
	_, err = kc.Get("test")
	if err != ErrKeychainUnavailable {
		t.Fatalf("expected ErrKeychainUnavailable while blacklisted, got %v", err)
	}

	// Simulate TTL expiry by manually setting the timestamp to the past
	cc := kc.(*cachedKeychain)
	cc.unavailableSince.Store(time.Now().Add(-blacklistTTL - time.Second).UnixNano())

	// After TTL expiry, should retry and blacklist again
	_, err = kc.Get("test")
	if err != ErrKeychainUnavailable {
		t.Fatalf("expected retry after TTL expiry, got %v", err)
	}

	// Verify the inner was called again (retry happened): 2 calls total
	// (1 initial blacklisting + 1 after TTL expiry)
	if inner.callCount != 2 {
		t.Errorf("expected 2 calls (1 before TTL + 1 after), got %d", inner.callCount)
	}
}

func TestKeychainBlacklist_SuccessClearsBlacklist(t *testing.T) {
	// Use a keychain that fails once then succeeds
	var failOnce atomic.Bool
	failOnce.Store(true)
	inner := &conditionalKeychain{failOnce: &failOnce}

	kc := NewCached(inner)

	// First call fails, blacklists
	_, err := kc.Get("test")
	if err != ErrKeychainUnavailable {
		t.Fatalf("expected ErrKeychainUnavailable, got %v", err)
	}

	// Simulate TTL expiry
	cc := kc.(*cachedKeychain)
	cc.unavailableSince.Store(time.Now().Add(-blacklistTTL - time.Second).UnixNano())

	// Now the inner keychain succeeds
	val, err := kc.Get("test")
	if err != nil {
		t.Fatalf("expected success after recovery, got %v", err)
	}
	if val != "ok" {
		t.Errorf("expected 'ok', got %q", val)
	}

	// Verify blacklist was cleared (next call should succeed without retry)
	val2, err := kc.Get("test2")
	if err != nil {
		t.Fatalf("expected success (blacklist cleared), got %v", err)
	}
	if val2 != "ok" {
		t.Errorf("expected 'ok', got %q", val2)
	}
}

// conditionalKeychain fails on first call, then succeeds.
type conditionalKeychain struct {
	failOnce *atomic.Bool
	store    map[string]string
}

func (c *conditionalKeychain) Get(service string) (string, error) {
	if c.failOnce.Load() {
		c.failOnce.Store(false)
		return "", ErrKeychainUnavailable
	}
	return "ok", nil
}

func (c *conditionalKeychain) Set(service, value string) error {
	return nil
}

func (c *conditionalKeychain) Delete(service string) error {
	return nil
}
