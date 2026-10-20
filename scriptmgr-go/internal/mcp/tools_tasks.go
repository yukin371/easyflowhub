package mcp

import (
	"context"
	"encoding/json"
	"time"

	"scriptmgr/internal/model"
)

// ============================================================================
// Async Task Tools
// ============================================================================

func (s *Server) callGetTaskResult(req Request) Response {
	if s.api == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "API not configured",
			},
		}
	}

	arguments, ok := req.Params["arguments"].(map[string]any)
	if !ok {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: "missing arguments",
			},
		}
	}

	// Validate
	if err := ValidateToolArgs("get_task_result", arguments); err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: err.Error(),
			},
		}
	}

	taskID := arguments["task_id"].(string)

	// Get wait parameter
	wait := false
	if w, ok := arguments["wait"].(bool); ok {
		wait = w
	}

	// If wait=true, poll until the task reaches a terminal state or the
	// bounded wait times out. This is a real, cancellable delay owned by the
	// task-waiter (F11); tests inject a fake sleeper to drive completion,
	// timeout, and cancellation without wall-clock delay.
	if wait {
		ctx, cancel := context.WithTimeout(context.Background(), taskWaitTimeout)
		defer cancel()
		return s.waitTaskResult(ctx, req, taskID, defaultTaskWaiter())
	}

	// Non-waiting: just return current task status
	task, err := s.api.GetTask(taskID)
	if err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: err.Error(),
			},
		}
	}

	resultJSON, _ := json.Marshal(task)
	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "text",
					"text": string(resultJSON),
				},
			},
		},
	}
}

func (s *Server) callReadLog(req Request) Response {
	if s.api == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "API not configured",
			},
		}
	}

	arguments, ok := req.Params["arguments"].(map[string]any)
	if !ok {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: "missing arguments",
			},
		}
	}

	// Validate
	if err := ValidateToolArgs("read_log", arguments); err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: err.Error(),
			},
		}
	}

	taskID := arguments["task_id"].(string)

	// Get optional parameters
	offset := 0
	limit := 100
	tail := false

	if o, ok := arguments["offset"].(float64); ok {
		offset = int(o)
	}
	if l, ok := arguments["limit"].(float64); ok {
		limit = int(l)
	}
	if t, ok := arguments["tail"].(bool); ok {
		tail = t
	}

	// Read log
	content, task, err := s.api.ReadTaskLog(taskID, offset, limit, tail)
	if err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: err.Error(),
			},
		}
	}

	result := map[string]any{
		"task_id":  taskID,
		"status":   task.Status,
		"content":  content,
		"offset":   offset,
		"limit":    limit,
		"tail":     tail,
		"log_path": task.OutputPath,
	}
	resultJSON, _ := json.Marshal(result)
	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "text",
					"text": string(resultJSON),
				},
			},
		},
	}
}

const (
	// taskWaitTimeout bounds how long get_task_result(wait=true) polls.
	taskWaitTimeout = 60 * time.Second
	// taskWaitInterval is the delay between polls.
	taskWaitInterval = 500 * time.Millisecond
)

// taskWaiter owns the polling delay for get_task_result. sleep is injectable
// so tests can drive completion, timeout, and cancellation deterministically.
type taskWaiter struct {
	interval time.Duration
	timeout  time.Duration
	sleep    func(ctx context.Context, d time.Duration) error
}

func defaultTaskWaiter() *taskWaiter {
	return &taskWaiter{
		interval: taskWaitInterval,
		timeout:  taskWaitTimeout,
		sleep: func(ctx context.Context, d time.Duration) error {
			timer := time.NewTimer(d)
			defer timer.Stop()
			select {
			case <-ctx.Done():
				return ctx.Err()
			case <-timer.C:
				return nil
			}
		},
	}
}

// isTerminalTaskStatus reports whether a task has stopped running.
func isTerminalTaskStatus(status string) bool {
	return status != "pending" && status != "running"
}

// waitCompletion polls get until it reports a terminal task, the timeout
// elapses, or ctx is cancelled. It returns the last observed task, a timedOut
// flag, and an error (nil for normal completion or timeout).
func (w *taskWaiter) waitCompletion(ctx context.Context, get func() (*model.Task, error)) (*model.Task, bool, error) {
	elapsed := time.Duration(0)
	for {
		task, err := get()
		if err != nil {
			return nil, false, err
		}
		if isTerminalTaskStatus(task.Status) {
			return task, false, nil
		}
		if elapsed >= w.timeout {
			return task, true, nil
		}
		if err := w.sleep(ctx, w.interval); err != nil {
			return task, false, err
		}
		elapsed += w.interval
	}
}

// waitTaskResult drives the waiting get_task_result path.
func (s *Server) waitTaskResult(ctx context.Context, req Request, taskID string, w *taskWaiter) Response {
	task, timedOut, err := w.waitCompletion(ctx, func() (*model.Task, error) {
		return s.api.GetTask(taskID)
	})
	if err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: err.Error(),
			},
		}
	}
	if timedOut {
		return textResultResponse(req.ID, map[string]any{
			"task_id":  taskID,
			"status":   task.Status,
			"timeout":  true,
			"message":  "Task still running after " + taskWaitTimeout.String() + " timeout",
			"log_path": task.OutputPath,
		})
	}
	return textResultResponse(req.ID, task)
}

// textResultResponse wraps a payload as an MCP text-content result.
func textResultResponse(id *int, payload any) Response {
	raw, _ := json.Marshal(payload)
	return Response{
		JSONRPC: "2.0",
		ID:      id,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "text",
					"text": string(raw),
				},
			},
		},
	}
}
