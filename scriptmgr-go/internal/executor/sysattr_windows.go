//go:build windows

package executor

import (
	"os/exec"
	"syscall"

	"golang.org/x/sys/windows"
)

func applySysProcAttr(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{
		HideWindow:    true,
		CreationFlags: 0x00000008 | 0x00000200, // CREATE_NO_WINDOW | CREATE_NEW_CONSOLE
	}
}

// windowsSysProcAttr returns Windows-specific process attributes
func windowsSysProcAttr() *windows.SysProcAttr {
	return &windows.SysProcAttr{
		HideWindow:    true,
		CreationFlags: 0x00000008 | 0x00000200,
	}
}
