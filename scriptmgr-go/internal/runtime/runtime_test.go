package runtime

import (
	"os"
	"path/filepath"
	"testing"
)

func TestNewRuntimeManager(t *testing.T) {
	tmpDir := t.TempDir()
	rm := NewRuntimeManager(tmpDir)

	if rm.stateDir != tmpDir {
		t.Errorf("expected stateDir %s, got %s", tmpDir, rm.stateDir)
	}

	expectedRunDir := filepath.Join(tmpDir, RunDirName)
	if rm.runDir != expectedRunDir {
		t.Errorf("expected runDir %s, got %s", expectedRunDir, rm.runDir)
	}
}

func TestAcquireAndReleaseLock(t *testing.T) {
	tmpDir := t.TempDir()
	rm := NewRuntimeManager(tmpDir)

	// First acquire should succeed
	if err := rm.AcquireLock(); err != nil {
		t.Fatalf("AcquireLock failed: %v", err)
	}

	// Second acquire should fail (lock held)
	rm2 := NewRuntimeManager(tmpDir)
	if err := rm2.AcquireLock(); err != ErrLockHeld {
		t.Errorf("expected ErrLockHeld, got %v", err)
	}

	// Release lock
	if err := rm.ReleaseLock(); err != nil {
		t.Fatalf("ReleaseLock failed: %v", err)
	}

	// Now acquire should succeed again
	if err := rm2.AcquireLock(); err != nil {
		t.Errorf("AcquireLock after release failed: %v", err)
	}
	rm2.ReleaseLock()
}

func TestWriteAndReadPort(t *testing.T) {
	tmpDir := t.TempDir()
	rm := NewRuntimeManager(tmpDir)

	// Write port
	if err := rm.WritePort(8765); err != nil {
		t.Fatalf("WritePort failed: %v", err)
	}

	// Read port
	port, err := rm.ReadPort()
	if err != nil {
		t.Fatalf("ReadPort failed: %v", err)
	}

	if port != 8765 {
		t.Errorf("expected port 8765, got %d", port)
	}
}

func TestWritePID(t *testing.T) {
	tmpDir := t.TempDir()
	rm := NewRuntimeManager(tmpDir)

	if err := rm.WritePID(12345); err != nil {
		t.Fatalf("WritePID failed: %v", err)
	}

	// Verify file content
	pidPath := filepath.Join(tmpDir, RunDirName, PidFileName)
	data, err := os.ReadFile(pidPath)
	if err != nil {
		t.Fatalf("failed to read PID file: %v", err)
	}

	if string(data) != "12345" {
		t.Errorf("expected PID file content '12345', got '%s'", string(data))
	}
}

func TestCheckExistingPID_NoFile(t *testing.T) {
	tmpDir := t.TempDir()
	rm := NewRuntimeManager(tmpDir)

	pid, running, err := rm.CheckExistingPID()
	if err != nil {
		t.Fatalf("CheckExistingPID failed: %v", err)
	}

	if pid != 0 || running != false {
		t.Errorf("expected (0, false), got (%d, %v)", pid, running)
	}
}

func TestCheckExistingPID_WithFile(t *testing.T) {
	tmpDir := t.TempDir()
	rm := NewRuntimeManager(tmpDir)

	// Write current process PID
	currentPid := os.Getpid()
	if err := rm.WritePID(currentPid); err != nil {
		t.Fatalf("WritePID failed: %v", err)
	}

	pid, running, err := rm.CheckExistingPID()
	if err != nil {
		t.Fatalf("CheckExistingPID failed: %v", err)
	}

	if pid != currentPid {
		t.Errorf("expected pid %d, got %d", currentPid, pid)
	}

	// Current process should be running
	if !running {
		t.Error("expected current process to be running")
	}
}

func TestCheckExistingPID_ZombieProcess(t *testing.T) {
	tmpDir := t.TempDir()
	rm := NewRuntimeManager(tmpDir)

	// Write a PID that definitely doesn't exist (very high number)
	// PIDs are typically < 4194304 on Linux, < 65536 on Windows
	zombiePid := 99999999
	if err := rm.WritePID(zombiePid); err != nil {
		t.Fatalf("WritePID failed: %v", err)
	}

	pid, running, err := rm.CheckExistingPID()
	if err != nil {
		t.Fatalf("CheckExistingPID failed: %v", err)
	}

	if pid != zombiePid {
		t.Errorf("expected pid %d, got %d", zombiePid, pid)
	}

	// Zombie process should not be running
	if running {
		t.Error("expected zombie process to not be running")
	}
}

func TestCleanupStaleFiles(t *testing.T) {
	tmpDir := t.TempDir()
	rm := NewRuntimeManager(tmpDir)

	// Create stale files
	rm.ensureRunDir()
	pidPath := filepath.Join(tmpDir, RunDirName, PidFileName)
	portPath := filepath.Join(tmpDir, RunDirName, PortFileName)

	os.WriteFile(pidPath, []byte("12345"), 0644)
	os.WriteFile(portPath, []byte("8765"), 0644)

	// Cleanup
	if err := rm.CleanupStaleFiles(); err != nil {
		t.Fatalf("CleanupStaleFiles failed: %v", err)
	}

	// Verify files are removed
	if _, err := os.Stat(pidPath); !os.IsNotExist(err) {
		t.Error("expected PID file to be removed")
	}
	if _, err := os.Stat(portPath); !os.IsNotExist(err) {
		t.Error("expected port file to be removed")
	}
}

func TestCleanup(t *testing.T) {
	tmpDir := t.TempDir()
	rm := NewRuntimeManager(tmpDir)

	// Setup: create all files
	rm.AcquireLock()
	rm.WritePID(12345)
	rm.WritePort(8765)

	// Cleanup
	if err := rm.Cleanup(); err != nil {
		t.Fatalf("Cleanup failed: %v", err)
	}

	// Verify all files are removed
	pidPath := filepath.Join(tmpDir, RunDirName, PidFileName)
	portPath := filepath.Join(tmpDir, RunDirName, PortFileName)
	lockPath := filepath.Join(tmpDir, RunDirName, LockFileName)

	if _, err := os.Stat(pidPath); !os.IsNotExist(err) {
		t.Error("expected PID file to be removed")
	}
	if _, err := os.Stat(portPath); !os.IsNotExist(err) {
		t.Error("expected port file to be removed")
	}
	if _, err := os.Stat(lockPath); !os.IsNotExist(err) {
		t.Error("expected lock file to be removed")
	}
}

func TestIsProcessRunning_CurrentProcess(t *testing.T) {
	tmpDir := t.TempDir()
	rm := NewRuntimeManager(tmpDir)

	// Current process should be running
	if !rm.IsProcessRunning(os.Getpid()) {
		t.Error("expected current process to be running")
	}
}

func TestIsProcessRunning_NonExistent(t *testing.T) {
	tmpDir := t.TempDir()
	rm := NewRuntimeManager(tmpDir)

	// Non-existent PID should not be running
	if rm.IsProcessRunning(99999999) {
		t.Error("expected non-existent process to not be running")
	}
}

func TestIsProcessRunning_InvalidPID(t *testing.T) {
	tmpDir := t.TempDir()
	rm := NewRuntimeManager(tmpDir)

	// Invalid PIDs should return false
	if rm.IsProcessRunning(0) {
		t.Error("expected PID 0 to not be running")
	}
	if rm.IsProcessRunning(-1) {
		t.Error("expected negative PID to not be running")
	}
}
