package runtime

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"syscall"
)

// Constants for runtime file names and directory
const (
	RunDirName   = "run"
	PidFileName  = "scriptmgr.pid"
	PortFileName = "scriptmgr.port"
	LockFileName = "scriptmgr.lock"
)

// Error types
var (
	ErrAlreadyRunning = errors.New("server already running")
	ErrLockHeld       = errors.New("another instance is starting")
)

// RuntimeInfo contains information about a running instance
type RuntimeInfo struct {
	Pid       int
	Port      int
	StartedAt int64 // Unix timestamp
}

// RuntimeManager manages PID/Port/Lock files for process lifecycle control
type RuntimeManager struct {
	stateDir string   // Base state directory (~/.scriptmgr)
	runDir   string   // Run directory (~/.scriptmgr/run)
	lockFile *os.File // Held while starting
}

// NewRuntimeManager creates a new runtime manager
func NewRuntimeManager(stateDir string) *RuntimeManager {
	return &RuntimeManager{
		stateDir: stateDir,
		runDir:   filepath.Join(stateDir, RunDirName),
	}
}

// ensureRunDir creates the run directory if it doesn't exist
func (rm *RuntimeManager) ensureRunDir() error {
	return os.MkdirAll(rm.runDir, 0o755)
}

// AcquireLock creates an exclusive lock file to prevent race conditions during startup
func (rm *RuntimeManager) AcquireLock() error {
	if err := rm.ensureRunDir(); err != nil {
		return fmt.Errorf("failed to create run directory: %w", err)
	}

	lockPath := filepath.Join(rm.runDir, LockFileName)

	// Try to create the lock file exclusively
	f, err := os.OpenFile(lockPath, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o644)
	if err != nil {
		if os.IsExist(err) {
			return ErrLockHeld
		}
		return fmt.Errorf("failed to create lock file: %w", err)
	}

	rm.lockFile = f
	return nil
}

// ReleaseLock releases the startup lock
func (rm *RuntimeManager) ReleaseLock() error {
	if rm.lockFile == nil {
		return nil
	}

	lockPath := rm.lockFile.Name()
	if err := rm.lockFile.Close(); err != nil {
		return err
	}
	rm.lockFile = nil

	return os.Remove(lockPath)
}

// IsProcessRunning checks if a process with the given PID is running
// Cross-platform: uses different approaches for Unix vs Windows
func (rm *RuntimeManager) IsProcessRunning(pid int) bool {
	if pid <= 0 {
		return false
	}

	if runtime.GOOS == "windows" {
		return rm.isProcessRunningWindows(pid)
	}
	return rm.isProcessRunningUnix(pid)
}

// isProcessRunningUnix checks if process is running on Unix systems
func (rm *RuntimeManager) isProcessRunningUnix(pid int) bool {
	// On Unix, os.FindProcess always succeeds, so we need to send signal 0
	// Signal 0 doesn't actually send a signal, just checks if process exists
	process, err := os.FindProcess(pid)
	if err != nil {
		return false
	}
	err = process.Signal(syscall.Signal(0))
	return err == nil
}

// isProcessRunningWindows checks if process is running on Windows
func (rm *RuntimeManager) isProcessRunningWindows(pid int) bool {
	// On Windows, we use tasklist command to check if process exists
	// tasklist /FI "PID eq 1234" /NH returns process info or "INFO: No tasks..."
	cmd := exec.Command("tasklist", "/FI", fmt.Sprintf("PID eq %d", pid), "/NH")
	output, err := cmd.Output()
	if err != nil {
		return false
	}
	// If output contains the PID, process is running
	return strings.Contains(string(output), strconv.Itoa(pid))
}

// CheckExistingPID reads the PID file and checks if that process is still running
// Returns: pid (from file), running (if process is alive), error
func (rm *RuntimeManager) CheckExistingPID() (pid int, running bool, err error) {
	pidPath := filepath.Join(rm.runDir, PidFileName)

	data, err := os.ReadFile(pidPath)
	if err != nil {
		if os.IsNotExist(err) {
			return 0, false, nil // No PID file, no existing instance
		}
		return 0, false, fmt.Errorf("failed to read PID file: %w", err)
	}

	// Parse PID from file content (tolerate trailing newline)
	pidStr := strings.TrimSpace(string(data))
	pid, err = strconv.Atoi(pidStr)
	if err != nil {
		return 0, false, fmt.Errorf("invalid PID in file: %w", err)
	}

	// Check if process is still running
	running = rm.IsProcessRunning(pid)
	return pid, running, nil
}

// CleanupStaleFiles removes stale PID and Port files from zombie processes
func (rm *RuntimeManager) CleanupStaleFiles() error {
	pidPath := filepath.Join(rm.runDir, PidFileName)
	portPath := filepath.Join(rm.runDir, PortFileName)

	// Remove PID file if exists
	if err := os.Remove(pidPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("failed to remove stale PID file: %w", err)
	}

	// Remove Port file if exists
	if err := os.Remove(portPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("failed to remove stale port file: %w", err)
	}

	return nil
}

// WritePID writes the current process PID to the PID file
func (rm *RuntimeManager) WritePID(pid int) error {
	if err := rm.ensureRunDir(); err != nil {
		return fmt.Errorf("failed to create run directory: %w", err)
	}

	pidPath := filepath.Join(rm.runDir, PidFileName)
	content := strconv.Itoa(pid)

	return os.WriteFile(pidPath, []byte(content), 0o644)
}

// WritePort writes the port number to the port file
func (rm *RuntimeManager) WritePort(port int) error {
	if err := rm.ensureRunDir(); err != nil {
		return fmt.Errorf("failed to create run directory: %w", err)
	}

	portPath := filepath.Join(rm.runDir, PortFileName)
	content := strconv.Itoa(port)

	return os.WriteFile(portPath, []byte(content), 0o644)
}

// ReadPort reads the port number from the port file
func (rm *RuntimeManager) ReadPort() (port int, err error) {
	portPath := filepath.Join(rm.runDir, PortFileName)

	data, err := os.ReadFile(portPath)
	if err != nil {
		return 0, fmt.Errorf("failed to read port file: %w", err)
	}

	portStr := strings.TrimSpace(string(data))
	port, err = strconv.Atoi(portStr)
	if err != nil {
		return 0, fmt.Errorf("invalid port in file: %w", err)
	}

	return port, nil
}

// Cleanup removes PID, Port, and Lock files on shutdown
func (rm *RuntimeManager) Cleanup() error {
	var errs []error

	// Remove PID file
	pidPath := filepath.Join(rm.runDir, PidFileName)
	if err := os.Remove(pidPath); err != nil && !os.IsNotExist(err) {
		errs = append(errs, fmt.Errorf("failed to remove PID file: %w", err))
	}

	// Remove Port file
	portPath := filepath.Join(rm.runDir, PortFileName)
	if err := os.Remove(portPath); err != nil && !os.IsNotExist(err) {
		errs = append(errs, fmt.Errorf("failed to remove port file: %w", err))
	}

	// Release and remove lock file
	if err := rm.ReleaseLock(); err != nil {
		errs = append(errs, fmt.Errorf("failed to release lock: %w", err))
	}

	if len(errs) > 0 {
		return fmt.Errorf("cleanup errors: %v", errs)
	}

	return nil
}
