//go:build windows

package keychain

import (
	"fmt"
	"regexp"
	"strings"
	"syscall"
	"unsafe"
)

var validServiceNameRe = regexp.MustCompile(`^[a-z]+$`)

type windowsKeychain struct{}

func init() {
	newFunc = newWindowsKeychain
}

func newWindowsKeychain() (Keychain, error) {
	return &windowsKeychain{}, nil
}

func (k *windowsKeychain) serviceName(service string) string {
	return strings.TrimPrefix(service, servicePrefix)
}

// Get retrieves a credential from Windows Credential Manager using CredReadW.
func (k *windowsKeychain) Get(service string) (string, error) {
	name := k.serviceName(service)
	if !validServiceNameRe.MatchString(name) {
		return "", fmt.Errorf("invalid service name: %s", service)
	}

	target := servicePrefix + name
	targetPtr, err := syscall.UTF16PtrFromString(target)
	if err != nil {
		return "", ErrKeychainUnavailable
	}

	var cred *credential
	ret, _, _ := credRead.Call(uintptr(unsafe.Pointer(targetPtr)), 1, 0, uintptr(unsafe.Pointer(&cred)))
	if ret == 0 {
		return "", ErrKeyNotFound
	}
	defer credFree.Call(uintptr(unsafe.Pointer(cred)))

	return syscall.UTF16ToString((*[1 << 20]uint16)(unsafe.Pointer(cred.credentialBlob))[:cred.credentialBlobSize/2]), nil
}

// Set stores a credential in Windows Credential Manager using CredWriteW.
func (k *windowsKeychain) Set(service, value string) error {
	name := k.serviceName(service)
	if !validServiceNameRe.MatchString(name) {
		return fmt.Errorf("invalid service name: %s", service)
	}

	target := servicePrefix + name
	targetPtr, err := syscall.UTF16PtrFromString(target)
	if err != nil {
		return ErrKeychainUnavailable
	}
	valuePtr, err := syscall.UTF16PtrFromString(value)
	if err != nil {
		return ErrKeychainUnavailable
	}

	var cred credential
	cred.flags = 0
	cred.persist = 2 // CRED_PERSIST_LOCAL_MACHINE
	cred.credType = 1 // CRED_TYPE_GENERIC
	cred.targetName = targetPtr
	cred.credentialBlob = (*byte)(unsafe.Pointer(valuePtr))
	cred.credentialBlobSize = uint32(len(value) * 2)

	ret, _, _ := credWrite.Call(uintptr(unsafe.Pointer(&cred)), 0)
	if ret == 0 {
		return ErrKeychainUnavailable
	}
	return nil
}

// Delete removes a credential from Windows Credential Manager using CredDeleteW.
func (k *windowsKeychain) Delete(service string) error {
	name := k.serviceName(service)
	if !validServiceNameRe.MatchString(name) {
		return fmt.Errorf("invalid service name: %s", service)
	}

	target := servicePrefix + name
	targetPtr, err := syscall.UTF16PtrFromString(target)
	if err != nil {
		return ErrKeychainUnavailable
	}

	ret, _, _ := credDelete.Call(uintptr(unsafe.Pointer(targetPtr)), 1, 0)
	if ret == 0 {
		return ErrKeyNotFound
	}
	return nil
}

// Windows CREDENTIAL struct (simplified for our use case)
type credential struct {
	flags              uint32
	credType           uint32
	targetName         *uint16
	comment            *uint16
	lastWritten        syscall.Filetime
	credentialBlobSize uint32
	credentialBlob     *byte
	persist            uint32
	attributeCount     uint32
	attributes         uintptr
	targetAlias        *uint16
	userName           *uint16
}

var (
	advapi32   = syscall.NewLazyDLL("advapi32.dll")
	credRead   = advapi32.NewProc("CredReadW")
	credWrite  = advapi32.NewProc("CredWriteW")
	credDelete = advapi32.NewProc("CredDeleteW")
	credFree   = advapi32.NewProc("CredFree")
)
