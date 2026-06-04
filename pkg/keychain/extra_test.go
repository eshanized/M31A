package keychain

import (
	"errors"
	"fmt"
	"os/exec"
	"testing"

	"github.com/godbus/dbus/v5"
)

// ---------------------------------------------------------------------------
// validateService tests
// ---------------------------------------------------------------------------

func TestValidateService_Valid(t *testing.T) {
	validNames := []string{"openrouter", "zen", "my-api-key", "anthropic", "a", "123", "test-key-123"}
	for _, name := range validNames {
		t.Run(name, func(t *testing.T) {
			if err := validateService(name); err != nil {
				t.Errorf("validateService(%q) should succeed, got %v", name, err)
			}
		})
	}
}

func TestValidateService_Invalid(t *testing.T) {
	invalidNames := []string{
		"",
		"OpenRouter",  // uppercase
		"my api key",  // spaces
		"key; rm -rf", // injection attempt
		"key/../../",  // path traversal
		"key=value",   // equals sign
		"key&value",   // ampersand
		"key|value",   // pipe
		"key`cmd`",    // backtick
		"key$(cmd)",   // dollar
		"hello world", // space
		"a.b",         // dot
		"a_b",         // underscore
	}
	for _, name := range invalidNames {
		t.Run(name, func(t *testing.T) {
			err := validateService(name)
			if err == nil {
				t.Errorf("validateService(%q) should fail, got nil", name)
			}
			if !errors.Is(err, ErrKeychainUnavailable) {
				t.Errorf("validateService(%q) should return ErrKeychainUnavailable, got %v", name, err)
			}
		})
	}
}

// ---------------------------------------------------------------------------
// isDBusUnavailable tests
// ---------------------------------------------------------------------------

func TestIsDBusUnavailable_NilError(t *testing.T) {
	if isDBusUnavailable(nil) {
		t.Error("isDBusUnavailable(nil) should return false")
	}
}

func TestIsDBusUnavailable_ConnectionRefused(t *testing.T) {
	err := fmt.Errorf("dial unix /run/user/1000/bus: connection refused")
	if !isDBusUnavailable(err) {
		t.Error("isDBusUnavailable should detect 'connection refused'")
	}
}

func TestIsDBusUnavailable_NoSuchFile(t *testing.T) {
	err := fmt.Errorf("open /run/user/1000/bus: no such file or directory")
	if !isDBusUnavailable(err) {
		t.Error("isDBusUnavailable should detect 'no such file or directory'")
	}
}

func TestIsDBusUnavailable_DbusKeyword(t *testing.T) {
	err := fmt.Errorf("dbus: some error occurred")
	if !isDBusUnavailable(err) {
		t.Error("isDBusUnavailable should detect 'dbus' in error string")
	}
}

func TestIsDBusUnavailable_UnrelatedError(t *testing.T) {
	err := fmt.Errorf("some unrelated error")
	if isDBusUnavailable(err) {
		t.Error("isDBusUnavailable should return false for unrelated errors")
	}
}

func TestIsDBusUnavailable_ExecError(t *testing.T) {
	// An exec.Error is not a D-Bus error
	err := &exec.Error{Name: "test", Err: fmt.Errorf("not found")}
	if isDBusUnavailable(err) {
		t.Error("isDBusUnavailable should return false for exec.Error")
	}
}

// ---------------------------------------------------------------------------
// isPassNotFound tests
// ---------------------------------------------------------------------------

func TestIsPassNotFound_NilError(t *testing.T) {
	if isPassNotFound(nil) {
		t.Error("isPassNotFound(nil) should return false")
	}
}

func TestIsPassNotFound_UnrelatedError(t *testing.T) {
	err := fmt.Errorf("some unrelated error")
	if isPassNotFound(err) {
		t.Error("isPassNotFound should return false for unrelated errors")
	}
}

func TestIsPassNotFound_StderrContainsNotFound(t *testing.T) {
	exitErr := &exec.ExitError{
		ProcessState: nil,
		Stderr:       []byte("Error: gpg decryption failed: not found in store"),
	}
	if !isPassNotFound(exitErr) {
		t.Error("isPassNotFound should detect 'not found' in stderr")
	}
}

func TestIsPassNotFound_StderrContainsNotIn(t *testing.T) {
	exitErr := &exec.ExitError{
		ProcessState: nil,
		Stderr:       []byte("Error: path is not in the password store"),
	}
	if !isPassNotFound(exitErr) {
		t.Error("isPassNotFound should detect 'not in' in stderr")
	}
}

func TestIsPassNotFound_StderrNoMatch(t *testing.T) {
	exitErr := &exec.ExitError{
		ProcessState: nil,
		Stderr:       []byte("gpg: decryption failed: bad passphrase"),
	}
	if isPassNotFound(exitErr) {
		t.Error("isPassNotFound should return false for GPG failure")
	}
}

// ---------------------------------------------------------------------------
// isPassGPGFailure tests
// ---------------------------------------------------------------------------

func TestIsPassGPGFailure_NilError(t *testing.T) {
	if isPassGPGFailure(nil) {
		t.Error("isPassGPGFailure(nil) should return false")
	}
}

func TestIsPassGPGFailure_UnrelatedError(t *testing.T) {
	err := fmt.Errorf("some unrelated error")
	if isPassGPGFailure(err) {
		t.Error("isPassGPGFailure should return false for unrelated errors")
	}
}

func TestIsPassGPGFailure_DecryptionFailed(t *testing.T) {
	exitErr := &exec.ExitError{
		ProcessState: nil,
		Stderr:       []byte("gpg: decryption failed: No secret key"),
	}
	if !isPassGPGFailure(exitErr) {
		t.Error("isPassGPGFailure should detect 'gpg: decryption failed'")
	}
}

func TestIsPassGPGFailure_BadPassphrase(t *testing.T) {
	exitErr := &exec.ExitError{
		ProcessState: nil,
		Stderr:       []byte("gpg: bad passphrase"),
	}
	if !isPassGPGFailure(exitErr) {
		t.Error("isPassGPGFailure should detect 'bad passphrase'")
	}
}

func TestIsPassGPGFailure_GpgError(t *testing.T) {
	exitErr := &exec.ExitError{
		ProcessState: nil,
		Stderr:       []byte("gpg: error reading file"),
	}
	if !isPassGPGFailure(exitErr) {
		t.Error("isPassGPGFailure should detect 'gpg: error'")
	}
}

func TestIsPassGPGFailure_NoGpgInStderr(t *testing.T) {
	exitErr := &exec.ExitError{
		ProcessState: nil,
		Stderr:       []byte("some other error"),
	}
	if isPassGPGFailure(exitErr) {
		t.Error("isPassGPGFailure should return false when stderr has no GPG indicators")
	}
}

// ---------------------------------------------------------------------------
// isPassUnavailable tests
// ---------------------------------------------------------------------------

func TestIsPassUnavailable_NilError(t *testing.T) {
	if isPassUnavailable(nil) {
		t.Error("isPassUnavailable(nil) should return false")
	}
}

func TestIsPassUnavailable_ExecError(t *testing.T) {
	// exec.Error means the binary wasn't found
	err := &exec.Error{Name: "pass", Err: fmt.Errorf("not found")}
	if !isPassUnavailable(err) {
		t.Error("isPassUnavailable should detect exec.Error")
	}
}

func TestIsPassUnavailable_ExitErrorNotInstalled(t *testing.T) {
	exitErr := &exec.ExitError{
		ProcessState: nil,
		Stderr:       []byte("pass: not found"),
	}
	if !isPassUnavailable(exitErr) {
		t.Error("isPassUnavailable should detect 'not found' in exit error stderr")
	}
}

func TestIsPassUnavailable_ExitErrorNotInstalled2(t *testing.T) {
	exitErr := &exec.ExitError{
		ProcessState: nil,
		Stderr:       []byte("pass is not installed"),
	}
	if !isPassUnavailable(exitErr) {
		t.Error("isPassUnavailable should detect 'not installed' in exit error stderr")
	}
}

func TestIsPassUnavailable_ExitErrorNoStore(t *testing.T) {
	exitErr := &exec.ExitError{
		ProcessState: nil,
		Stderr:       []byte("Error: No password store"),
	}
	if !isPassUnavailable(exitErr) {
		t.Error("isPassUnavailable should detect 'No password store'")
	}
}

func TestIsPassUnavailable_ExitErrorEmptyStore(t *testing.T) {
	exitErr := &exec.ExitError{
		ProcessState: nil,
		Stderr:       []byte("password-store is empty"),
	}
	if !isPassUnavailable(exitErr) {
		t.Error("isPassUnavailable should detect 'password-store is empty'")
	}
}

func TestIsPassUnavailable_UnrelatedExitError(t *testing.T) {
	exitErr := &exec.ExitError{
		ProcessState: nil,
		Stderr:       []byte("gpg: decryption failed"),
	}
	if isPassUnavailable(exitErr) {
		t.Error("isPassUnavailable should return false for unrelated exit errors")
	}
}

func TestIsPassUnavailable_UnrelatedError(t *testing.T) {
	err := fmt.Errorf("some unrelated error")
	if isPassUnavailable(err) {
		t.Error("isPassUnavailable should return false for unrelated errors")
	}
}

// ---------------------------------------------------------------------------
// fmtSecret tests
// ---------------------------------------------------------------------------

func TestFmtSecret(t *testing.T) {
	sessionPath := dbus.ObjectPath("/org/freedesktop/secrets/session/abc123")
	value := "my-api-key"

	result := fmtSecret(sessionPath, value)

	if result["session"] != sessionPath {
		t.Errorf("expected session %q, got %v", sessionPath, result["session"])
	}
	if string(result["value"].([]byte)) != value {
		t.Errorf("expected value %q, got %v", value, result["value"])
	}
	if result["content_type"] != "text/plain" {
		t.Errorf("expected content_type 'text/plain', got %v", result["content_type"])
	}
	params, ok := result["parameters"].([]byte)
	if !ok {
		t.Fatal("expected parameters to be []byte")
	}
	if len(params) != 0 {
		t.Errorf("expected empty parameters, got %d bytes", len(params))
	}
}

// ---------------------------------------------------------------------------
// New() and Keychain interface tests
// ---------------------------------------------------------------------------

func TestNew_ReturnsKeychain(t *testing.T) {
	kc, err := New()
	if err != nil {
		t.Fatalf("New() failed: %v", err)
	}
	if kc == nil {
		t.Fatal("New() returned nil")
	}
	var _ Keychain = kc
}

func TestKeychain_Constants(t *testing.T) {
	if AccountName != "m31a" {
		t.Errorf("AccountName = %q, want %q", AccountName, "m31a")
	}
	if servicePrefix != "m31a/" {
		t.Errorf("servicePrefix = %q, want %q", servicePrefix, "m31a/")
	}
}

// ---------------------------------------------------------------------------
// Error sentinel tests
// ---------------------------------------------------------------------------

func TestErrors_AreDefined(t *testing.T) {
	errs := []error{
		ErrKeychainUnavailable,
		ErrKeyNotFound,
		ErrKeychainDecrypt,
		ErrNotImplemented,
	}
	for _, err := range errs {
		if err == nil {
			t.Errorf("error sentinel should not be nil")
		}
		if err.Error() == "" {
			t.Errorf("error sentinel should have non-empty message")
		}
	}
}

func TestErrKeychainUnavailable_IsError(t *testing.T) {
	if !errors.Is(ErrKeychainUnavailable, ErrKeychainUnavailable) {
		t.Error("ErrKeychainUnavailable should match itself")
	}
}

func TestErrKeyNotFound_IsError(t *testing.T) {
	if !errors.Is(ErrKeyNotFound, ErrKeyNotFound) {
		t.Error("ErrKeyNotFound should match itself")
	}
}

func TestErrKeychainDecrypt_IsError(t *testing.T) {
	wrapped := fmt.Errorf("operation failed: %w", ErrKeychainDecrypt)
	if !errors.Is(wrapped, ErrKeychainDecrypt) {
		t.Error("wrapped ErrKeychainDecrypt should match with errors.Is")
	}
}

// ---------------------------------------------------------------------------
// linuxKeychain method tests (validate fallback path behavior)
// ---------------------------------------------------------------------------

func TestLinuxKeychain_Get_InvalidService(t *testing.T) {
	kc, err := New()
	if err != nil {
		t.Fatalf("New() failed: %v", err)
	}

	_, err = kc.Get("INVALID-SERVICE")
	if err == nil {
		t.Error("Get with invalid service should fail")
	}
	if !errors.Is(err, ErrKeychainUnavailable) {
		t.Errorf("expected ErrKeychainUnavailable, got %v", err)
	}
}

func TestLinuxKeychain_Set_InvalidService(t *testing.T) {
	kc, err := New()
	if err != nil {
		t.Fatalf("New() failed: %v", err)
	}

	err = kc.Set("INVALID-SERVICE", "value")
	if err == nil {
		t.Error("Set with invalid service should fail")
	}
	if !errors.Is(err, ErrKeychainUnavailable) {
		t.Errorf("expected ErrKeychainUnavailable, got %v", err)
	}
}

func TestLinuxKeychain_Delete_InvalidService(t *testing.T) {
	kc, err := New()
	if err != nil {
		t.Fatalf("New() failed: %v", err)
	}

	err = kc.Delete("INVALID-SERVICE")
	if err == nil {
		t.Error("Delete with invalid service should fail")
	}
	if !errors.Is(err, ErrKeychainUnavailable) {
		t.Errorf("expected ErrKeychainUnavailable, got %v", err)
	}
}

// validServiceName regex tests
func TestValidServiceName_Regex(t *testing.T) {
	tests := []struct {
		input string
		valid bool
	}{
		{"openrouter", true},
		{"zen", true},
		{"my-api-key", true},
		{"key123", true},
		{"123", true},
		{"OpenRouter", false},
		{"my api", false},
		{"key; rm -rf", false},
		{"", false},
		{"a.b", false},
		{"a_b", false},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			got := validServiceName.MatchString(tt.input)
			if got != tt.valid {
				t.Errorf("validServiceName(%q) = %v, want %v", tt.input, got, tt.valid)
			}
		})
	}
}
