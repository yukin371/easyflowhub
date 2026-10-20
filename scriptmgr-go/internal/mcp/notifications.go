package mcp

import (
	"encoding/json"
	"log/slog"
)

// NotificationParams contains the parameters for task_completed notification
type NotificationParams struct {
	TaskID              string `json:"task_id"`
	ScriptID            string `json:"script_id"`
	Status              string `json:"status"`
	DurationMs          int64  `json:"duration_ms"`
	ExitCode            *int   `json:"exit_code,omitempty"`
	OutputSummary       string `json:"output_summary,omitempty"`
	OutputFullAvailable bool   `json:"output_full_available"`
}

// Notification represents a JSON-RPC 2.0 notification
type Notification struct {
	JSONRPC string             `json:"jsonrpc"`
	Method  string             `json:"method"`
	Params  NotificationParams `json:"params"`
}

// BroadcastTaskStatus implements executor.StatusBroadcaster interface
// It sends a notification when an async task completes
func (s *Server) BroadcastTaskStatus(taskID, scriptID, status string, exitCode *int, durationMs int64) {
	// Get output summary from API if available
	var outputSummary string
	var outputFullAvailable bool
	if s.api != nil {
		if task, err := s.api.GetTask(taskID); err == nil && task != nil {
			outputSummary = task.OutputSummary
			outputFullAvailable = task.OutputPath != ""
		}
	}

	notification := Notification{
		JSONRPC: "2.0",
		Method:  "notifications/task_completed",
		Params: NotificationParams{
			TaskID:              taskID,
			ScriptID:            scriptID,
			Status:              status,
			DurationMs:          durationMs,
			ExitCode:            exitCode,
			OutputSummary:       outputSummary,
			OutputFullAvailable: outputFullAvailable,
		},
	}

	// Send notification if transport is available
	if s.transport != nil {
		notificationBytes, err := json.Marshal(notification)
		if err != nil {
			slog.Error("failed to marshal notification", "error", err)
			return
		}
		if err := s.transport.WriteMessage(notificationBytes); err != nil {
			slog.Error("failed to send notification", "error", err)
		}
	} else {
		// Graceful degradation - log to stdout for debugging
		slog.Debug("notification skipped - no transport", "task_id", taskID, "status", status)
	}
}
