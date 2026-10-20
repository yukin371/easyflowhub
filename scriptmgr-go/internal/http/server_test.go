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
	TrashEntries     []model.ScriptTrashEntry
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

// Script file management stubs: the HTTP tests only assert routing and status
// codes, so these return zero values unless a test overrides them.
func (m *MockAPI) ListRootDetails() ([]string, []string, error) {
	return m.Roots, m.Roots, m.Err
}

func (m *MockAPI) AddRoot(path string) (string, error) {
	return path, m.Err
}

func (m *MockAPI) RemoveRoot(path string) error {
	return m.Err
}

func (m *MockAPI) ReadScriptSource(scriptID string) (model.ScriptSource, error) {
	return model.ScriptSource{ID: scriptID}, m.Err
}

func (m *MockAPI) WriteScriptSource(scriptID, content string) (model.ScriptSource, error) {
	return model.ScriptSource{ID: scriptID, Content: content}, m.Err
}

func (m *MockAPI) CreateScript(req model.CreateScriptRequest) (model.ScriptRecord, error) {
	return model.ScriptRecord{ID: req.Filename, Path: req.Filename}, m.Err
}

func (m *MockAPI) ImportScripts(req model.ImportScriptsRequest) (model.ImportScriptsResponse, error) {
	return model.ImportScriptsResponse{OK: m.Err == nil, Imported: []model.ScriptRecord{}, Failed: []model.ImportFailure{}}, m.Err
}

func (m *MockAPI) DeleteScript(scriptID string) (model.ScriptTrashEntry, error) {
	return model.ScriptTrashEntry{ID: scriptID}, m.Err
}

func (m *MockAPI) ListScriptTrash() ([]model.ScriptTrashEntry, error) {
	return m.TrashEntries, m.Err
}

func (m *MockAPI) RestoreScriptTrash(entryID string) (model.ScriptTrashEntry, error) {
	return model.ScriptTrashEntry{ID: entryID}, m.Err
}

func (m *MockAPI) PurgeScriptTrash(entryID string) error {
	return m.Err
}

func (m *MockAPI) EmptyScriptTrash() (int, error) {
	return 0, m.Err
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

func TestHandleListScriptTrashKeepsValidEntriesWhenOneIsCorrupt(t *testing.T) {
	mock := &MockAPI{
		TrashEntries: []model.ScriptTrashEntry{{ID: "valid-entry", Name: "keep.py"}},
		Err:          errors.New("trash contains invalid entries: broken: malformed manifest"),
	}
	srv := NewServer(mock)
	req := httptest.NewRequest("GET", "/api/trash/scripts", nil)
	rec := httptest.NewRecorder()

	srv.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("expected partial trash response status 200, got %d", rec.Code)
	}
	var resp model.ScriptTrashResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &resp); err != nil {
		t.Fatalf("parse response: %v", err)
	}
	if !resp.OK || len(resp.Entries) != 1 || resp.Entries[0].ID != "valid-entry" {
		t.Fatalf("valid entries were not preserved: %+v", resp)
	}
	if resp.Warning == "" {
		t.Fatal("expected corruption warning in partial response")
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
	extensionDir := filepath.Join(stateDir, "extensions", "secret-provider")
	if err := os.MkdirAll(extensionDir, 0o755); err != nil {
		t.Fatalf("MkdirAll extension dir failed: %v", err)
	}
	if err := os.WriteFile(filepath.Join(extensionDir, "plugin.json"), []byte(`{
  "id": "secret-provider",
  "name": "Secret Provider",
  "version": "1.0.0",
  "contributions": {
    "relay_providers": [{
      "id": "extension-provider",
      "name": "Extension Provider",
      "base_url": "https://extension.example.com",
      "headers": {"X-API-Key": "extension-manifest-secret"}
    }]
  }
}`), 0o644); err != nil {
		t.Fatalf("WriteFile extension manifest failed: %v", err)
	}
	relayService, err := relay.NewService(stateDir)
	if err != nil {
		t.Fatalf("NewService failed: %v", err)
	}
	if err := relayService.SaveConfig(relay.Config{
		Version: 1,
		Providers: []relay.Provider{
			{ID: "primary", Name: "Primary", BaseURL: "https://api.example.com", APIKey: "scriptmgr-api-secret", Headers: map[string]string{"Authorization": "Bearer scriptmgr-header-secret"}, Enabled: true},
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
	for _, secret := range []string{"scriptmgr-api-secret", "scriptmgr-header-secret", "extension-manifest-secret"} {
		if strings.Contains(rec.Body.String(), secret) {
			t.Fatalf("relay config GET leaked secret %q", secret)
		}
	}

	var response struct {
		OK       bool           `json:"ok"`
		Snapshot relay.Snapshot `json:"snapshot"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &response); err != nil {
		t.Fatalf("json decode failed: %v", err)
	}
	if !response.OK || len(response.Snapshot.Config.Providers) != 1 {
		t.Fatalf("unexpected relay response: %+v", response)
	}
	if got := response.Snapshot.Config.Providers[0].APIKey; got != "__EASYFLOWHUB_REDACTED__" {
		t.Fatalf("expected main HTTP API to redact API key, got %q", got)
	}
	if len(response.Snapshot.Extensions) != 1 || response.Snapshot.Extensions[0].Manifest.Contributions.RelayProviders[0].Headers["X-API-Key"] != "__EASYFLOWHUB_REDACTED__" {
		t.Fatalf("expected extension snapshot headers to be redacted, got %+v", response.Snapshot.Extensions)
	}
	body, err := json.Marshal(response.Snapshot.Config)
	if err != nil {
		t.Fatalf("marshal redacted relay config failed: %v", err)
	}
	putReq := httptest.NewRequest(http.MethodPut, "/api/relay/config", strings.NewReader(string(body)))
	putRec := httptest.NewRecorder()
	srv.ServeHTTP(putRec, putReq)
	if putRec.Code != http.StatusOK {
		t.Fatalf("expected relay config PUT 200, got %d: %s", putRec.Code, putRec.Body.String())
	}
	for _, secret := range []string{"scriptmgr-api-secret", "scriptmgr-header-secret", "extension-manifest-secret"} {
		if strings.Contains(putRec.Body.String(), secret) {
			t.Fatalf("relay config PUT leaked secret %q", secret)
		}
	}
	saved, err := relayService.LoadConfig()
	if err != nil {
		t.Fatalf("LoadConfig after redacted PUT failed: %v", err)
	}
	if saved.Providers[0].APIKey != "scriptmgr-api-secret" || saved.Providers[0].Headers["Authorization"] != "Bearer scriptmgr-header-secret" {
		t.Fatalf("redacted config PUT lost saved secrets: %+v", saved.Providers[0])
	}
}

func TestHandleListExtensions(t *testing.T) {
	stateDir := t.TempDir()
	root := filepath.Join(stateDir, "extensions", "sample")
	if err := os.MkdirAll(root, 0o755); err != nil {
		t.Fatalf("MkdirAll failed: %v", err)
	}
	if err := os.WriteFile(filepath.Join(root, "plugin.json"), []byte(`{
  "id":"sample",
  "name":"Sample",
  "version":"1.0.0",
  "contributions":{"relay_providers":[{"id":"sample-provider","name":"Sample Provider","base_url":"https://ext.example.com","headers":{"X-API-Key":"extension-api-secret"}}]}
}`), 0o644); err != nil {
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
	if strings.Contains(rec.Body.String(), "extension-api-secret") || !strings.Contains(rec.Body.String(), "__EASYFLOWHUB_REDACTED__") {
		t.Fatalf("extension list did not redact provider credentials: %s", rec.Body.String())
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
	contributionsReq := httptest.NewRequest(http.MethodGet, "/api/extensions/contributions", nil)
	contributionsRec := httptest.NewRecorder()
	srv.ServeHTTP(contributionsRec, contributionsReq)
	if contributionsRec.Code != http.StatusOK {
		t.Fatalf("expected contributions 200, got %d: %s", contributionsRec.Code, contributionsRec.Body.String())
	}
	if strings.Contains(contributionsRec.Body.String(), "extension-api-secret") || !strings.Contains(contributionsRec.Body.String(), "__EASYFLOWHUB_REDACTED__") {
		t.Fatalf("extension contributions did not redact provider credentials: %s", contributionsRec.Body.String())
	}
}

func TestHandleListExtensionContributions(t *testing.T) {
	stateDir := t.TempDir()
	root := filepath.Join(stateDir, "extensions", "sample")
	if err := os.MkdirAll(root, 0o755); err != nil {
		t.Fatalf("MkdirAll failed: %v", err)
	}
	if err := os.WriteFile(filepath.Join(root, "plugin.json"), []byte(`{
  "id":"sample",
  "name":"Sample",
  "version":"1.0.0",
  "contributions":{
    "relay_providers":[
      {"id":"provider-ext","name":"Extension Provider","base_url":"https://ext.example.com","headers":{"Authorization":"Bearer contribution-secret"}}
    ]
  }
}`), 0o644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	relayService, err := relay.NewService(stateDir)
	if err != nil {
		t.Fatalf("NewService failed: %v", err)
	}

	srv := NewServerWithRelay(&MockAPI{}, relayService)
	req := httptest.NewRequest(http.MethodGet, "/api/extensions/contributions", nil)
	rec := httptest.NewRecorder()

	srv.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rec.Code, rec.Body.String())
	}

	var response struct {
		OK            bool `json:"ok"`
		Contributions struct {
			RelayProviders []struct {
				ID     string `json:"id"`
				Source struct {
					ExtensionID string `json:"extension_id"`
				} `json:"source"`
			} `json:"relay_providers"`
		} `json:"contributions"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &response); err != nil {
		t.Fatalf("json decode failed: %v", err)
	}
	if !response.OK || len(response.Contributions.RelayProviders) != 1 {
		t.Fatalf("unexpected contributions response: %+v", response)
	}
	if response.Contributions.RelayProviders[0].ID != "provider-ext" {
		t.Fatalf("unexpected provider payload: %+v", response.Contributions.RelayProviders[0])
	}
	if strings.Contains(rec.Body.String(), "contribution-secret") || !strings.Contains(rec.Body.String(), "__EASYFLOWHUB_REDACTED__") {
		t.Fatalf("extension contribution secret was not redacted: %s", rec.Body.String())
	}
}

func TestHandleListMCPServers(t *testing.T) {
	stateDir := t.TempDir()
	mcpConfigPath := filepath.Join(stateDir, "mcp-config.json")
	t.Setenv("SCRIPTMGR_MCP_CONFIG", mcpConfigPath)
	if err := os.WriteFile(mcpConfigPath, []byte(`{
  "version": "1.0",
  "servers": {
    "persisted-server": {
      "command": "node",
      "args": ["persisted.js"]
    },
    "conflict-server": {
      "command": "python",
      "args": ["persisted.py"]
    }
  }
}`), 0o644); err != nil {
		t.Fatalf("WriteFile mcp config failed: %v", err)
	}

	root := filepath.Join(stateDir, "extensions", "sample")
	if err := os.MkdirAll(root, 0o755); err != nil {
		t.Fatalf("MkdirAll failed: %v", err)
	}
	if err := os.WriteFile(filepath.Join(root, "plugin.json"), []byte(`{
  "id":"sample",
  "name":"Sample",
  "version":"1.0.0",
  "contributions":{
    "mcp_servers":[
      {"id":"extension-server","name":"Extension Server","command":"deno","args":["run","server.ts"]},
      {"id":"conflict-server","name":"Conflict Server","command":"node"}
    ]
  }
}`), 0o644); err != nil {
		t.Fatalf("WriteFile manifest failed: %v", err)
	}

	relayService, err := relay.NewService(stateDir)
	if err != nil {
		t.Fatalf("NewService failed: %v", err)
	}

	srv := NewServerWithRelay(&MockAPI{}, relayService)
	req := httptest.NewRequest(http.MethodGet, "/api/mcp/servers", nil)
	rec := httptest.NewRecorder()

	srv.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rec.Code, rec.Body.String())
	}

	var response struct {
		OK      bool `json:"ok"`
		Catalog struct {
			Servers []struct {
				Key          string `json:"key"`
				Name         string `json:"name"`
				Status       string `json:"status"`
				Source       string `json:"source"`
				ConflictWith string `json:"conflict_with"`
			} `json:"servers"`
		} `json:"catalog"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &response); err != nil {
		t.Fatalf("json decode failed: %v", err)
	}
	if !response.OK || len(response.Catalog.Servers) != 4 {
		t.Fatalf("unexpected mcp server response: %+v", response)
	}

	foundExtension := false
	foundConflict := false
	for _, item := range response.Catalog.Servers {
		if item.Key == "extension:sample:extension-server" && item.Status == "extension" && item.Source == "extension:sample" {
			foundExtension = true
		}
		if item.Key == "extension:sample:conflict-server" && item.Status == "conflicted" && item.ConflictWith == "persisted:conflict-server" {
			foundConflict = true
		}
	}
	if !foundExtension || !foundConflict {
		t.Fatalf("missing extension/conflict entries: %+v", response.Catalog.Servers)
	}
}
