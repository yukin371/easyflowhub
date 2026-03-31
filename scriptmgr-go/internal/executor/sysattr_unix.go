//go:build !windows

package executor

import (
	"os/exec"
)

func applySysProcAttr(cmd *exec.Cmd) {
	// No-op on non-Windows platforms
}
