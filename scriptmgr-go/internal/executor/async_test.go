package executor

import (
	"testing"
)

// MockBroadcaster captures broadcast calls for testing
type MockBroadcaster struct {
	Calls []BroadcastCall
}

type BroadcastCall struct {
	TaskID     string
	ScriptID   string
	Status     string
	ExitCode   *int
	DurationMs int64
}

func (m *MockBroadcaster) BroadcastTaskStatus(taskID, scriptID, status string, exitCode *int, durationMs int64) {
	m.Calls = append(m.Calls, BroadcastCall{
		TaskID:     taskID,
		ScriptID:   scriptID,
		Status:     status,
		ExitCode:   exitCode,
		DurationMs: durationMs,
	})
}

func TestAsyncManagerSetBroadcaster(t *testing.T) {
	// This test verifies that AsyncManager can have a broadcaster set
	// The actual broadcast calls happen in Start, RunWorker, Cancel, and refreshSessions
	// which require more complex setup with store and discovery

	mock := &MockBroadcaster{}

	// Create an AsyncManager (with nil store/discovery just to test the interface)
	manager := &AsyncManager{
		store:     nil,
		discovery: nil,
	}

	// SetBroadcaster should work without panic
	manager.SetBroadcaster(mock)

	// Verify broadcaster was set
	if manager.broadcaster == nil {
		t.Error("broadcaster should be set after SetBroadcaster call")
	}
}

func TestBroadcastStatusWithNilBroadcaster(t *testing.T) {
	manager := &AsyncManager{
		store:       nil,
		discovery:   nil,
		broadcaster: nil,
	}

	// This should not panic even with nil broadcaster
	manager.broadcastStatus("task-1", "script-1", "running", nil, 0)
}

func TestBroadcastStatusWithBroadcaster(t *testing.T) {
	mock := &MockBroadcaster{}
	manager := &AsyncManager{
		store:       nil,
		discovery:   nil,
		broadcaster: mock,
	}

	exitCode := 0
	manager.broadcastStatus("task-1", "script-1", "success", &exitCode, 1500)

	if len(mock.Calls) != 1 {
		t.Fatalf("Expected 1 broadcast call, got %d", len(mock.Calls))
	}

	call := mock.Calls[0]
	if call.TaskID != "task-1" {
		t.Errorf("Expected TaskID task-1, got %s", call.TaskID)
	}
	if call.ScriptID != "script-1" {
		t.Errorf("Expected ScriptID script-1, got %s", call.ScriptID)
	}
	if call.Status != "success" {
		t.Errorf("Expected Status success, got %s", call.Status)
	}
	if call.ExitCode == nil || *call.ExitCode != 0 {
		t.Errorf("Expected ExitCode 0, got %v", call.ExitCode)
	}
	if call.DurationMs != 1500 {
		t.Errorf("Expected DurationMs 1500, got %d", call.DurationMs)
	}
}
