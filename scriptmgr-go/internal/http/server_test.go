package http

import (
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"scriptmgr/internal/model"
	"scriptmgr/internal/relay"
)

var ErrNotFound = errors.New("not found")

// MockAPI implements the API interface for testing
type MockAPI struct {
	Scripts          []model.ScriptRecord
	Roots            []string
	Err              error
	RunScriptRes     any
	RunScriptErr     error
	Task             *model.Task
	TaskErr          error
	Tasks            []model.Task
	TasksErr         error
	LogContent       string
	LogTask          *model.Task
	LogErr           error
	CancelSessionRes model.SessionRecord
	CancelErr        error
}

func (m *MockAPI) ListScripts(search string) ([]model.ScriptRecord, []string, error) {
	return m.Scripts, m.Roots, m.Err
}

func (m *MockAPI) DescribeScript(scriptID string) (model.ScriptRecord, error) {
	if len(m.Scripts) > 0 {
		return m.Scripts[0], m.Err
	}
	return model.ScriptRecord{}, m.Err
}

func (m *MockAPI) RunScript(scriptID string, args []string, opts model.RunOptions) (any, error) {
	return m.RunScriptRes, m.RunScriptErr
}

func (m *MockAPI) GetTask(taskID string) (*model.Task, error) {
	return m.Task, m.TaskErr
}

func (m *MockAPI) ListTasks(status string, limit int) ([]model.Task, error) {
	return m.Tasks, m.TasksErr
}

func (m *MockAPI) ReadTaskLog(taskID string, offset, limit int, tail bool) (string, *model.Task, error) {
	return m.LogContent, m.LogTask, m.LogErr
}

func (m *MockAPI) CancelSession(sessionID string) (model.SessionRecord, error) {
	return m.CancelSessionRes, m.CancelErr
}

func (m *MockAPI) CancelTask(taskID string) (model.SessionRecord, error) {
	return m.CancelSessionRes, m.CancelErr
}

func (m *MockAPI) UpdateScriptCategory(scriptID, category string) error {
	return m.Err
}

func (m *MockAPI) ListCategories() (map[string]int, error) {
	return make(map[string]int), nil
}

func TestNewServer(t *testing.T) {
	mock := &MockAPI{}
	srv := NewServer(mock)

	if srv == nil {
		t.Fatal("expected server, got nil")
	}
}

func TestHandleListScripts(t *testing.T) {
	mock := &MockAPI{
		Scripts: []model.ScriptRecord{
			{ID: "test-1", Name: "Test Script", ScriptType: "powershell"},
		},
		Roots: []string{"C:\\Scripts"},
	}

	srv := NewServer(mock)
	req := httptest.NewRequest("GET", "/api/scripts", nil)
	rec := httptest.NewRecorder()

	srv.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", rec.Code)
	}

	var resp map[string]any
	if err := json.Unmarshal(rec.Body.Bytes(), &resp); err != nil {
		t.Fatalf("failed to parse response: %v", err)
	}

	if resp["ok"] != true {
		t.Errorf("expected ok=true, got %v", resp["ok"])
	}

	scripts, ok := resp["scripts"].([]any)
	if !ok || len(scripts) == 0 {
		t.Errorf("expected scripts array with items, got %v", resp["scripts"])
	}
}

func TestHandleGetTask(t *testing.T) {
	mock := &MockAPI{
		Task: &model.Task{
			TaskID:   "task-123",
			ScriptID: "test-script",
			Status:   "completed",
		},
	}

	srv := NewServer(mock)
	req := httptest.NewRequest("GET", "/api/tasks/task-123", nil)
	rec := httptest.NewRecorder()

	srv.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", rec.Code)
	}

	var resp map[string]any
	if err := json.Unmarshal(rec.Body.Bytes(), &resp); err != nil {
		t.Fatalf("failed to parse response: %v", err)
	}

	if resp["task_id"] != "task-123" {
		t.Errorf("expected task_id=task-123, got %v", resp["task_id"])
	}
}

func TestHandleRunScript(t *testing.T) {
	mock := &MockAPI{
		RunScriptRes: map[string]any{
			"task_id":   "task-456",
			"script_id": "test-script",
			"status":    "running",
		},
	}

	srv := NewServer(mock)

	body := `{"script_id": "test-script", "args": [], "async": true}`
	req := httptest.NewRequest("POST", "/api/run", strings.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	rec := httptest.NewRecorder()

	srv.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d: %s", rec.Code, rec.Body.String())
	}

	var resp map[string]any
	if err := json.Unmarshal(rec.Body.Bytes(), &resp); err != nil {
		t.Fatalf("failed to parse response: %v", err)
	}

	if resp["task_id"] != "task-456" {
		t.Errorf("expected task_id in response, got %v", resp)
	}
}

func TestHandleRunScript_MissingID(t *testing.T) {
	mock := &MockAPI{}
	srv := NewServer(mock)

	body := `{"args": []}`
	req := httptest.NewRequest("POST", "/api/run", strings.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	rec := httptest.NewRecorder()

	srv.ServeHTTP(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Errorf("expected status 400, got %d", rec.Code)
	}
}

func TestHandleListTasks(t *testing.T) {
	mock := &MockAPI{
		Tasks: []model.Task{
			{TaskID: "task-1", ScriptID: "script-a", Status: "completed"},
			{TaskID: "task-2", ScriptID: "script-b", Status: "running"},
		},
	}

	srv := NewServer(mock)
	req := httptest.NewRequest("GET", "/api/tasks?status=running&limit=10", nil)
	rec := httptest.NewRecorder()

	srv.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", rec.Code)
	}

	var resp map[string]any
	if err := json.Unmarshal(rec.Body.Bytes(), &resp); err != nil {
		t.Fatalf("failed to parse response: %v", err)
	}

	if resp["ok"] != true {
		t.Errorf("expected ok=true, got %v", resp["ok"])
	}
}

func TestHandleTaskLog(t *testing.T) {
	mock := &MockAPI{
		LogContent: "line1\nline2\nline3",
		LogTask:    &model.Task{TaskID: "task-1", Status: "completed"},
	}

	srv := NewServer(mock)
	req := httptest.NewRequest("GET", "/api/tasks/task-1/log?offset=0&limit=100", nil)
	rec := httptest.NewRecorder()

	srv.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", rec.Code)
	}

	var resp map[string]any
	if err := json.Unmarshal(rec.Body.Bytes(), &resp); err != nil {
		t.Fatalf("failed to parse response: %v", err)
	}

	if resp["ok"] != true {
		t.Errorf("expected ok=true, got %v", resp["ok"])
	}

	if resp["content"] != "line1\nline2\nline3" {
		t.Errorf("expected log content, got %v", resp["content"])
	}
}

func TestHandleCancel(t *testing.T) {
	mock := &MockAPI{
		CancelSessionRes: model.SessionRecord{
			SessionID: "session-1",
			Status:    "cancelled",
		},
	}

	srv := NewServer(mock)
	req := httptest.NewRequest("POST", "/api/cancel/session-1", nil)
	rec := httptest.NewRecorder()

	srv.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", rec.Code)
	}

	var resp map[string]any
	if err := json.Unmarshal(rec.Body.Bytes(), &resp); err != nil {
		t.Fatalf("failed to parse response: %v", err)
	}

	if resp["ok"] != true {
		t.Errorf("expected ok=true, got %v", resp["ok"])
	}
}

func TestHandleGetTask_NotFound(t *testing.T) {
	mock := &MockAPI{
		TaskErr: ErrNotFound,
	}

	srv := NewServer(mock)
	req := httptest.NewRequest("GET", "/api/tasks/nonexistent", nil)
	rec := httptest.NewRecorder()

	srv.ServeHTTP(rec, req)

	if rec.Code != http.StatusNotFound {
		t.Errorf("expected status 404, got %d", rec.Code)
	}
}

func TestHandleRelayGetConfig(t *testing.T) {
	stateDir := t.TempDir()
	relayService, err := relay.NewService(stateDir)
	if err != nil {
		t.Fatalf("NewService failed: %v", err)
	}
	if err := relayService.SaveConfig(relay.Config{
		Version: 1,
		Providers: []relay.Provider{
			{ID: "primary", Name: "Primary", BaseURL: "https://api.example.com", Enabled: true},
		},
		Routes: []relay.Route{
			{ID: "default", PathPrefixes: []string{"/v1/"}, ProviderIDs: []string{"primary"}},
		},
	}); err != nil {
		t.Fatalf("SaveConfig failed: %v", err)
	}

	srv := NewServerWithRelay(&MockAPI{}, relayService)
	req := httptest.NewRequest(http.MethodGet, "/api/relay/config", nil)
	rec := httptest.NewRecorder()

	srv.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rec.Code, rec.Body.String())
	}

	var response struct {
		OK       bool `json:"ok"`
		Snapshot struct {
			Config struct {
				Providers []relay.Provider `json:"providers"`
			} `json:"config"`
		} `json:"snapshot"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &response); err != nil {
		t.Fatalf("json decode failed: %v", err)
	}
	if !response.OK || len(response.Snapshot.Config.Providers) != 1 {
		t.Fatalf("unexpected relay response: %+v", response)
	}
}

func TestHandleListExtensions(t *testing.T) {
	stateDir := t.TempDir()
	root := filepath.Join(stateDir, "extensions", "sample")
	if err := os.MkdirAll(root, 0o755); err != nil {
		t.Fatalf("MkdirAll failed: %v", err)
	}
	if err := os.WriteFile(filepath.Join(root, "plugin.json"), []byte(`{"id":"sample","name":"Sample","version":"1.0.0"}`), 0o644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	relayService, err := relay.NewService(stateDir)
	if err != nil {
		t.Fatalf("NewService failed: %v", err)
	}

	srv := NewServerWithRelay(&MockAPI{}, relayService)
	req := httptest.NewRequest(http.MethodGet, "/api/extensions", nil)
	rec := httptest.NewRecorder()

	srv.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rec.Code, rec.Body.String())
	}

	var response struct {
		OK         bool `json:"ok"`
		Count      int  `json:"count"`
		Extensions []struct {
			Status   string `json:"status"`
			Manifest *struct {
				ID string `json:"id"`
			} `json:"manifest"`
		} `json:"extensions"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &response); err != nil {
		t.Fatalf("json decode failed: %v", err)
	}
	if !response.OK || response.Count != 1 {
		t.Fatalf("unexpected extension response: %+v", response)
	}
	if response.Extensions[0].Manifest == nil || response.Extensions[0].Manifest.ID != "sample" {
		t.Fatalf("unexpected manifest payload: %+v", response.Extensions[0].Manifest)
	}
}
