package http

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"scriptmgr/internal/mcp"
	"scriptmgr/internal/model"
)

// F09: the HTTP load/unload endpoints must drive the real MCP tool router,
// not a parallel boolean map. Loading a category has to register an actual
// tool that tools/call would serve; unloading has to deregister it. The
// reported "loaded" flag and the set of live tool schemas must always agree.
func TestMCPLoadUnloadDrivesRealRouter(t *testing.T) {
	api := &MockAPI{
		Scripts: []model.ScriptRecord{
			{ID: "demo-script", Name: "demo", Path: "demo.py", ScriptType: "python", Category: "demo"},
		},
	}
	srv := NewServer(api)

	// sanitizeToolName prefixes "script_" and turns hyphens into underscores.
	toolName := "script_demo_script"
	if toolNameInSchemas(srv.router.GetToolSchemas(), toolName) {
		t.Fatalf("tool %q must not be registered before load", toolName)
	}

	// Load the category: a real tool must appear and the list must agree.
	req := httptest.NewRequest(http.MethodPost, "/api/mcp/load/demo", nil)
	rec := httptest.NewRecorder()
	srv.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("load: expected 200, got %d: %s", rec.Code, rec.Body.String())
	}
	if !toolNameInSchemas(srv.router.GetToolSchemas(), toolName) {
		t.Fatalf("load must register tool %q in the router", toolName)
	}
	if !routerReportsLoaded(t, srv, "demo", true) {
		t.Fatal("list must report demo as loaded right after load")
	}

	// Unload the category: the tool must disappear and the list must agree.
	req = httptest.NewRequest(http.MethodPost, "/api/mcp/unload/demo", nil)
	rec = httptest.NewRecorder()
	srv.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("unload: expected 200, got %d: %s", rec.Code, rec.Body.String())
	}
	if toolNameInSchemas(srv.router.GetToolSchemas(), toolName) {
		t.Fatalf("unload must deregister tool %q from the router", toolName)
	}
	if !routerReportsLoaded(t, srv, "demo", false) {
		t.Fatal("list must report demo as unloaded right after unload")
	}

	// Loading a category with no scripts must fail rather than fake success.
	req = httptest.NewRequest(http.MethodPost, "/api/mcp/load/ghost", nil)
	rec = httptest.NewRecorder()
	srv.ServeHTTP(rec, req)
	if rec.Code != http.StatusNotFound {
		t.Fatalf("loading an empty category must 404, got %d", rec.Code)
	}
}

func toolNameInSchemas(schemas []mcp.ToolWithSchema, name string) bool {
	for _, schema := range schemas {
		if schema.Name == name {
			return true
		}
	}
	return false
}

type mcpCategoryEntry struct {
	Name   string `json:"name"`
	Loaded bool   `json:"loaded"`
}

func routerReportsLoaded(t *testing.T, srv *Server, category string, want bool) bool {
	t.Helper()
	req := httptest.NewRequest(http.MethodGet, "/api/mcp/categories", nil)
	rec := httptest.NewRecorder()
	srv.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("list: expected 200, got %d: %s", rec.Code, rec.Body.String())
	}
	var response struct {
		OK         bool               `json:"ok"`
		Categories []mcpCategoryEntry `json:"categories"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &response); err != nil {
		t.Fatalf("decode: %v", err)
	}
	for _, item := range response.Categories {
		if item.Name == category {
			return item.Loaded == want
		}
	}
	return !want
}
