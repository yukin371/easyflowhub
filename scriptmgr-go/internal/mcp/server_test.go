package mcp

import (
	"encoding/json"
	"testing"

	"scriptmgr/internal/model"
)

// MockTransport captures messages for testing
type MockTransport struct {
	Messages [][]byte
}

func (m *MockTransport) WriteMessage(msg []byte) error {
	m.Messages = append(m.Messages, msg)
	return nil
}

// MockAPI implements ScriptAPI for testing
type MockAPI struct {
	Scripts         []model.ScriptRecord
	Roots           []string
	Err             error
	RunScriptRes    any
	RunScriptErr    error
	RunScriptCalled bool
	GetTaskRes      *model.Task
	GetTaskErr      error
}

func (m *MockAPI) ListScripts(search string) ([]model.ScriptRecord, []string, error) {
	return m.Scripts, m.Roots, m.Err
}

func (m *MockAPI) RunScript(scriptID string, args []string, opts model.RunOptions) (any, error) {
	m.RunScriptCalled = true
	return m.RunScriptRes, m.RunScriptErr
}

func (m *MockAPI) GetTask(taskID string) (*model.Task, error) {
	return m.GetTaskRes, m.GetTaskErr
}

func (m *MockAPI) ListTasks(status string, limit int) ([]model.Task, error) {
	return nil, nil
}

func (m *MockAPI) ReadTaskLog(taskID string, offset, limit int, tail bool) (string, *model.Task, error) {
	return "", nil, nil
}

func ptr(i int) *int {
	return &i
}

func TestServer_Initialize(t *testing.T) {
	srv := New()

	req := Request{
		JSONRPC: "2.0",
		ID:      ptr(1),
		Method:  "initialize",
		Params: map[string]any{
			"protocolVersion": "2024-11-05",
			"clientInfo": map[string]any{
				"name":    "test-client",
				"version": "1.0.0",
			},
		},
	}

	resp := srv.Handle(req)

	if resp.JSONRPC != "2.0" {
		t.Errorf("expected jsonrpc 2.0, got %s", resp.JSONRPC)
	}
	if resp.ID == nil || *resp.ID != 1 {
		t.Errorf("expected id 1, got %v", resp.ID)
	}
	if resp.Error != nil {
		t.Fatalf("unexpected error: %v", resp.Error)
	}

	result, ok := resp.Result.(map[string]any)
	if !ok {
		t.Fatalf("expected result to be map, got %T", resp.Result)
	}

	// Verify server info
	serverInfo, ok := result["serverInfo"].(map[string]any)
	if !ok {
		t.Fatalf("expected serverInfo in result")
	}
	if serverInfo["name"] != "scriptmgr" {
		t.Errorf("expected server name 'scriptmgr', got %v", serverInfo["name"])
	}

	// Verify protocol version
	if result["protocolVersion"] != "2024-11-05" {
		t.Errorf("expected protocol version 2024-11-05, got %v", result["protocolVersion"])
	}

	// Verify capabilities
	caps, ok := result["capabilities"].(map[string]any)
	if !ok {
		t.Fatalf("expected capabilities in result")
	}
	if _, hasTools := caps["tools"]; !hasTools {
		t.Errorf("expected tools capability")
	}
}

func TestServer_ToolsList(t *testing.T) {
	srv := New()

	req := Request{
		JSONRPC: "2.0",
		ID:      ptr(2),
		Method:  "tools/list",
		Params:  map[string]any{},
	}

	resp := srv.Handle(req)

	if resp.Error != nil {
		t.Fatalf("unexpected error: %v", resp.Error)
	}

	result, ok := resp.Result.(map[string]any)
	if !ok {
		t.Fatalf("expected result to be map, got %T", resp.Result)
	}

	tools, ok := result["tools"].([]ToolWithSchema)
	if !ok {
		t.Fatalf("expected tools array, got %T", result["tools"])
	}

	if len(tools) == 0 {
		t.Errorf("expected at least one tool")
	}
	found := false
	for _, tool := range tools {
		if tool.Name == "list_scripts" {
			found = true
			// Verify inputSchema is present
			if tool.InputSchema == nil {
				t.Errorf("expected inputSchema for list_scripts")
			}
			break
		}
	}
	if !found {
		t.Errorf("expected list_scripts tool")
	}
}

func TestServer_ToolsCall_ListScripts(t *testing.T) {
	mock := &MockAPI{
		Scripts: []model.ScriptRecord{
			{ID: "test-script-1", Name: "Test Script", Description: "A test", Category: "test", Tags: []string{"demo"}},
		},
		Roots: []string{"root1", "root2"},
		Err:   nil,
	}

	srv := New().WithAPI(mock)

	req := Request{
		JSONRPC: "2.0",
		ID:      ptr(4),
		Method:  "tools/call",
		Params: map[string]any{
			"name":      "list_scripts",
			"arguments": map[string]any{},
		},
	}

	resp := srv.Handle(req)

	if resp.Error != nil {
		t.Fatalf("unexpected error: %v", resp.Error)
	}

	result, ok := resp.Result.(map[string]any)
	if !ok {
		t.Fatalf("expected result to be map, got %T", resp.Result)
	}

	content, ok := result["content"].([]map[string]any)
	if !ok {
		t.Fatalf("expected content array in result, got %T", result["content"])
	}

	text, ok := content[0]["text"].(string)
	if len(text) < 10 {
		t.Errorf("expected text to contain script data, got: %s", text)
	}
}

func TestServer_ToolsCall_RunScript(t *testing.T) {
	mock := &MockAPI{
		Scripts: []model.ScriptRecord{
			{ID: "test-script-1", Name: "Test Script", Description: "A test", Category: "test", Tags: []string{"demo"}},
		},
		Roots: []string{"root1", "root2"},
		Err:   nil,
		RunScriptRes: map[string]any{
			"task_id":  "task-123",
			"status":   "running",
			"script_id": "test-script-1",
		},
	}

	srv := New().WithAPI(mock)

	req := Request{
		JSONRPC: "2.0",
		ID:      ptr(5),
		Method:  "tools/call",
		Params: map[string]any{
			"name": "run_script",
			"arguments": map[string]any{
				"script_id": "test-script-1",
				"async":     true,
				"dry_run":   false,
			},
		},
	}

	resp := srv.Handle(req)

	if resp.Error != nil {
		t.Fatalf("unexpected error: %v", resp.Error)
	}

	// Verify the API was called
	if !mock.RunScriptCalled {
		t.Error("expected RunScript to be called")
	}

	result, ok := resp.Result.(map[string]any)
	if !ok {
		t.Fatalf("expected result to be map, got %T", resp.Result)
	}

	content, ok := result["content"].([]map[string]any)
	if !ok {
		t.Fatalf("expected content array in result, got %T", result["content"])
	}

	if len(content) == 0 {
		t.Fatal("expected content in response")
	}

	text, ok := content[0]["text"].(string)
	if !ok || len(text) == 0 {
		t.Errorf("expected text content, got: %v", content[0]["text"])
	}
}

func TestServer_BroadcastTaskStatus(t *testing.T) {
	// Create a mock transport to capture output
	mockTransport := &MockTransport{}

	// Create mock API with task info
	mockAPI := &MockAPI{
		GetTaskRes: &model.Task{
			TaskID:        "task-123",
			ScriptID:      "test-script-1",
			Status:        "succeeded",
			OutputSummary: "Renamed 42 files",
			OutputPath:    "/path/to/output.log",
		},
	}

	// Create server with API and transport
	srv := New().WithAPI(mockAPI)
	srv.SetTransport(mockTransport)

	// Call BroadcastTaskStatus
	exitCode := 0
	srv.BroadcastTaskStatus("task-123", "test-script-1", "succeeded", &exitCode, 3500)

	// Verify notification was sent
	if len(mockTransport.Messages) != 1 {
		t.Fatalf("expected 1 message, got %d", len(mockTransport.Messages))
	}

	// Parse the notification
	var notification map[string]any
	if err := json.Unmarshal(mockTransport.Messages[0], &notification); err != nil {
		t.Fatalf("failed to parse notification: %v", err)
	}

	// Verify notification structure
	if notification["jsonrpc"] != "2.0" {
		t.Errorf("expected jsonrpc 2.0, got %v", notification["jsonrpc"])
	}
	if notification["method"] != "notifications/task_completed" {
		t.Errorf("expected method 'notifications/task_completed', got %v", notification["method"])
	}

	params, ok := notification["params"].(map[string]any)
	if !ok {
		t.Fatalf("expected params to be map, got %T", notification["params"])
	}

	// Verify params
	if params["task_id"] != "task-123" {
		t.Errorf("expected task_id 'task-123', got %v", params["task_id"])
	}
	if params["script_id"] != "test-script-1" {
		t.Errorf("expected script_id 'test-script-1', got %v", params["script_id"])
	}
	if params["status"] != "succeeded" {
		t.Errorf("expected status 'succeeded', got %v", params["status"])
	}
	if params["duration_ms"].(float64) != 3500 {
		t.Errorf("expected duration_ms 3500, got %v", params["duration_ms"])
	}
	if params["exit_code"].(float64) != 0 {
		t.Errorf("expected exit_code 0, got %v", params["exit_code"])
	}
	if params["output_summary"] != "Renamed 42 files" {
		t.Errorf("expected output_summary 'Renamed 42 files', got %v", params["output_summary"])
	}
	if params["output_full_available"] != true {
		t.Errorf("expected output_full_available true, got %v", params["output_full_available"])
	}
}

func TestServer_BroadcastTaskStatus_NoTransport(t *testing.T) {
	// Create mock API
	mockAPI := &MockAPI{
		GetTaskRes: &model.Task{
			TaskID:        "task-456",
			ScriptID:      "test-script-2",
			Status:        "failed",
			OutputSummary: "Error occurred",
		},
	}

	// Create server without transport
	srv := New().WithAPI(mockAPI)
	// Don't set transport

	// BroadcastTaskStatus should not panic when transport is nil
	exitCode := 1
	srv.BroadcastTaskStatus("task-456", "test-script-2", "failed", &exitCode, 1000)

	// If we get here without panic, the test passes
}
