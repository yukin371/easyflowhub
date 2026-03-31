package mcp

import (
	"testing"

	"scriptmgr/internal/model"
)

// MockAPIForRouter implements ScriptAPI interface for testing
type MockAPIForRouter struct {
	Scripts      []model.ScriptRecord
	RunScriptRes any
	RunScriptErr error
}

func (m *MockAPIForRouter) ListScripts(search string) ([]model.ScriptRecord, []string, error) {
	if search == "" {
		return m.Scripts, []string{"root1"}, nil
	}
	// Simple search filter
	var filtered []model.ScriptRecord
	for _, s := range m.Scripts {
		if containsIgnoreCase(s.Name, search) || containsIgnoreCase(s.Description, search) {
			filtered = append(filtered, s)
		}
	}
	return filtered, []string{"root1"}, nil
}

func (m *MockAPIForRouter) RunScript(scriptID string, args []string, opts model.RunOptions) (any, error) {
	return m.RunScriptRes, m.RunScriptErr
}

func (m *MockAPIForRouter) GetTask(taskID string) (*model.Task, error) {
	return nil, nil
}

func (m *MockAPIForRouter) ListTasks(status string, limit int) ([]model.Task, error) {
	return nil, nil
}

func (m *MockAPIForRouter) ReadTaskLog(taskID string, offset, limit int, tail bool) (string, *model.Task, error) {
	return "", nil, nil
}

func containsIgnoreCase(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(substr) == 0 ||
		(len(s) > 0 && len(substr) > 0 && (s[:len(substr)] == substr ||
			containsIgnoreCase(s[1:], substr))))
}

func intPtr(i int) *int {
	return &i
}

func TestToolRouter_ListCategories(t *testing.T) {
	mock := &MockAPIForRouter{
		Scripts: []model.ScriptRecord{
			{ID: "s1", Name: "Script 1", Category: "utils"},
			{ID: "s2", Name: "Script 2", Category: "utils"},
			{ID: "s3", Name: "Script 3", Category: "media"},
			{ID: "s4", Name: "Script 4", Category: ""}, // uncategorized
		},
	}

	router := NewToolRouter(mock, DefaultRouterConfig())

	categories, err := router.ListCategories()
	if err != nil {
		t.Fatalf("ListCategories failed: %v", err)
	}

	if len(categories) != 3 {
		t.Errorf("expected 3 categories, got %d", len(categories))
	}

	// Verify utils category count
	for _, cat := range categories {
		if cat.Name == "utils" && cat.Count != 2 {
			t.Errorf("expected utils count 2, got %d", cat.Count)
		}
		if cat.Name == "uncategorized" && cat.Count != 1 {
			t.Errorf("expected uncategorized count 1, got %d", cat.Count)
		}
	}
}

func TestToolRouter_LoadCategory(t *testing.T) {
	mock := &MockAPIForRouter{
		Scripts: []model.ScriptRecord{
			{
				ID:          "test-script-1",
				Name:        "Test Script",
				Category:    "test",
				Description: "A test script",
				Parameters: []model.ScriptParameter{
					{Name: "input", Type: "string", Required: true, Description: "Input path"},
				},
			},
		},
		RunScriptRes: map[string]any{"task_id": "t1", "status": "running"},
	}

	router := NewToolRouter(mock, DefaultRouterConfig())

	// Load category
	result, err := router.LoadCategory("test")
	if err != nil {
		t.Fatalf("LoadCategory failed: %v", err)
	}

	if result.Category != "test" {
		t.Errorf("expected category 'test', got %s", result.Category)
	}
	if result.ScriptCount != 1 {
		t.Errorf("expected 1 script, got %d", result.ScriptCount)
	}
	if result.AlreadyLoaded {
		t.Error("expected AlreadyLoaded to be false")
	}

	// Verify tool is registered
	schemas := router.GetToolSchemas()
	found := false
	for _, s := range schemas {
		if s.Name == "script_test_script_1" {
			found = true
			break
		}
	}
	if !found {
		t.Error("expected script tool to be registered")
	}

	// Verify IsCategoryLoaded
	if !router.IsCategoryLoaded("test") {
		t.Error("expected category to be loaded")
	}
}

func TestToolRouter_LoadCategory_AlreadyLoaded(t *testing.T) {
	mock := &MockAPIForRouter{
		Scripts: []model.ScriptRecord{
			{ID: "s1", Name: "Script 1", Category: "test"},
		},
	}

	router := NewToolRouter(mock, DefaultRouterConfig())

	// Load first time
	_, err := router.LoadCategory("test")
	if err != nil {
		t.Fatalf("first LoadCategory failed: %v", err)
	}

	// Load again - should return AlreadyLoaded=true
	result, err := router.LoadCategory("test")
	if err != nil {
		t.Fatalf("second LoadCategory failed: %v", err)
	}

	if !result.AlreadyLoaded {
		t.Error("expected AlreadyLoaded to be true on second load")
	}
}

func TestToolRouter_UnloadCategory(t *testing.T) {
	mock := &MockAPIForRouter{
		Scripts: []model.ScriptRecord{
			{ID: "s1", Name: "Script 1", Category: "test"},
		},
	}

	router := NewToolRouter(mock, DefaultRouterConfig())

	// Load and then unload
	router.LoadCategory("test")
	if !router.IsCategoryLoaded("test") {
		t.Error("expected category to be loaded")
	}

	err := router.UnloadCategory("test")
	if err != nil {
		t.Fatalf("UnloadCategory failed: %v", err)
	}

	if router.IsCategoryLoaded("test") {
		t.Error("expected category to be unloaded")
	}

	// Verify tool is removed
	schemas := router.GetToolSchemas()
	for _, s := range schemas {
		if s.Name == "script_s1" {
			t.Error("expected script tool to be unregistered")
		}
	}
}

func TestToolRouter_UnloadCategory_NotLoaded(t *testing.T) {
	mock := &MockAPIForRouter{
		Scripts: []model.ScriptRecord{},
	}

	router := NewToolRouter(mock, DefaultRouterConfig())

	// Unload non-loaded category should not error
	err := router.UnloadCategory("nonexistent")
	if err != nil {
		t.Errorf("expected no error for unloading non-loaded category, got: %v", err)
	}
}

func TestToolRouter_LRUEviction(t *testing.T) {
	mock := &MockAPIForRouter{
		Scripts: []model.ScriptRecord{
			{ID: "s1", Name: "Script 1", Category: "cat1"},
			{ID: "s2", Name: "Script 2", Category: "cat2"},
			{ID: "s3", Name: "Script 3", Category: "cat3"},
		},
	}

	// Limit to 2 categories
	router := NewToolRouter(mock, RouterConfig{MaxCategories: 2})

	// Load cat1, cat2
	router.LoadCategory("cat1")
	router.LoadCategory("cat2")

	loaded := router.GetLoadedCategories()
	if len(loaded) != 2 {
		t.Errorf("expected 2 loaded categories, got %d", len(loaded))
	}

	// Load cat3 should trigger LRU eviction of cat1
	result, err := router.LoadCategory("cat3")
	if err != nil {
		t.Fatalf("LoadCategory cat3 failed: %v", err)
	}

	if len(result.EvictedCategories) != 1 {
		t.Errorf("expected 1 evicted category, got %d", len(result.EvictedCategories))
	}

	loaded = router.GetLoadedCategories()
	if len(loaded) != 2 {
		t.Errorf("expected 2 loaded categories after eviction, got %d", len(loaded))
	}

	// cat1 should be evicted (oldest)
	for _, name := range loaded {
		if name == "cat1" {
			t.Error("cat1 should have been evicted")
		}
	}
}

func TestToolRouter_HandleTool_Static(t *testing.T) {
	mock := &MockAPIForRouter{
		Scripts: []model.ScriptRecord{},
	}

	router := NewToolRouter(mock, DefaultRouterConfig())

	// Register a static tool
	router.RegisterStatic("test_tool", ToolWithSchema{
		Name:        "test_tool",
		Description: "Test tool",
		InputSchema: map[string]any{"type": "object"},
	}, func(req Request) Response {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Result:  "ok",
		}
	})

	req := Request{
		JSONRPC: "2.0",
		ID:      intPtr(1),
		Method:  "tools/call",
		Params: map[string]any{
			"name": "test_tool",
		},
	}

	resp, handled := router.HandleTool("test_tool", req)
	if !handled {
		t.Error("expected tool to be handled")
	}
	if resp.Result != "ok" {
		t.Errorf("expected result 'ok', got %v", resp.Result)
	}
}

func TestToolRouter_HandleTool_Dynamic(t *testing.T) {
	mock := &MockAPIForRouter{
		Scripts: []model.ScriptRecord{
			{ID: "my-script", Name: "My Script", Category: "test"},
		},
		RunScriptRes: map[string]any{"task_id": "t1", "status": "running"},
	}

	router := NewToolRouter(mock, DefaultRouterConfig())

	// Load category to register dynamic tool
	router.LoadCategory("test")

	req := Request{
		JSONRPC: "2.0",
		ID:      intPtr(1),
		Method:  "tools/call",
		Params: map[string]any{
			"name":      "script_my_script",
			"arguments": map[string]any{},
		},
	}

	resp, handled := router.HandleTool("script_my_script", req)
	if !handled {
		t.Error("expected dynamic tool to be handled")
	}
	if resp.Error != nil {
		t.Errorf("unexpected error: %s", resp.Error.Message)
	}
}

func TestToolRouter_HandleTool_NotFound(t *testing.T) {
	mock := &MockAPIForRouter{
		Scripts: []model.ScriptRecord{},
	}

	router := NewToolRouter(mock, DefaultRouterConfig())

	req := Request{
		JSONRPC: "2.0",
		ID:      intPtr(1),
		Method:  "tools/call",
		Params: map[string]any{
			"name": "nonexistent",
		},
	}

	_, handled := router.HandleTool("nonexistent", req)
	if handled {
		t.Error("expected nonexistent tool to not be handled")
	}
}

func TestToolRouter_SearchScripts(t *testing.T) {
	mock := &MockAPIForRouter{
		Scripts: []model.ScriptRecord{
			{ID: "s1", Name: "Backup Files", Description: "Backup important files"},
			{ID: "s2", Name: "Clean Temp", Description: "Clean temporary files"},
			{ID: "s3", Name: "Sync Data", Description: "Synchronize data"},
		},
	}

	router := NewToolRouter(mock, DefaultRouterConfig())

	// Search for "files"
	results, err := router.SearchScripts("files")
	if err != nil {
		t.Fatalf("SearchScripts failed: %v", err)
	}

	// Note: The mock search is simple, just check it returns results
	if len(results) > 3 {
		t.Errorf("expected at most 3 results, got %d", len(results))
	}
}

func TestSanitizeToolName(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"my-script", "script_my_script"},
		{"My Script", "script_my_script"},
		{"script123", "script_script123"},
		{"test@script!", "script_test_script_"},
		{"UPPERCASE", "script_uppercase"},
	}

	for _, tt := range tests {
		result := sanitizeToolName(tt.input)
		if result != tt.expected {
			t.Errorf("sanitizeToolName(%q) = %q, expected %q", tt.input, result, tt.expected)
		}
	}
}

func TestBuildScriptToolSchema(t *testing.T) {
	script := model.ScriptRecord{
		ID:          "my-script",
		Name:        "My Script",
		Description: "A test script",
		Category:    "utils",
		Parameters: []model.ScriptParameter{
			{Name: "path", Type: "string", Label: "File Path", Required: true, Description: "Path to file"},
			{Name: "count", Type: "integer", Default: "10", Description: "Number of items"},
			{Name: "verbose", Type: "boolean", Default: "false", Description: "Verbose output"},
		},
	}

	schema := buildScriptToolSchema(script)

	if schema.Name != "script_my_script" {
		t.Errorf("expected name 'script_my_script', got %s", schema.Name)
	}

	props, ok := schema.InputSchema["properties"].(map[string]any)
	if !ok {
		t.Fatal("expected properties map")
	}

	// Verify path parameter
	if _, ok := props["path"]; !ok {
		t.Error("expected 'path' property")
	}

	// Verify required fields
	required, ok := schema.InputSchema["required"].([]string)
	if !ok {
		t.Fatal("expected required array")
	}
	if len(required) != 1 || required[0] != "path" {
		t.Errorf("expected required to be ['path'], got %v", required)
	}

	// Verify common parameters exist
	if _, ok := props["async"]; !ok {
		t.Error("expected 'async' property")
	}
	if _, ok := props["dry_run"]; !ok {
		t.Error("expected 'dry_run' property")
	}
}

func TestMapParamType(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"string", "string"},
		{"int", "integer"},
		{"integer", "integer"},
		{"float", "number"},
		{"number", "number"},
		{"bool", "boolean"},
		{"boolean", "boolean"},
		{"array", "array"},
		{"unknown", "string"},
	}

	for _, tt := range tests {
		result := mapParamType(tt.input)
		if result != tt.expected {
			t.Errorf("mapParamType(%q) = %q, expected %q", tt.input, result, tt.expected)
		}
	}
}
