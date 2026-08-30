//go:build windows

package testorch

import (
	"os"
	"syscall"
	"unsafe"
)

func hideFile(path string) error {
	ptr, err := syscall.UTF16PtrFromString(path)
	if err != nil {
		return err
	}
	_, _, e := syscall.NewLazyDLL("kernel32.dll").NewProc("SetFileAttributesW").Call(
		uintptr(unsafe.Pointer(ptr)),
		uintptr(0x02),
	)
	if e != 0 {
		return os.NewSyscallError("SetFileAttributesW", e)
	}
	return nil
}
