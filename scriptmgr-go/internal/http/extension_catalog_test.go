package http

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"scriptmgr/internal/extensions"
)

// stubCatalog implements ExtensionCatalog without any relay dependency, so
// the test can prove extension and MCP-catalog queries do not detour through
// relay initialisation (F13).
type stubCatalog struct{
	items     []extensions.ListedExtension
	roots     []string
	contrib   extensions.EffectiveContributions
	err       error
}

func (c *stubCatalog) ListExtensionsForAPI() ([]extensions.ListedExtension, []string, error) {
	return c.items, c.roots, c.err
}

func (c *stubCatalog) EffectiveContributionsForAPI() (extensions.EffectiveContributions, error) {
	return c.contrib, c.err
}

func (c *stubCatalog) EffectiveContributions() (extensions.EffectiveContributions, error) {
	return c.contrib, c.err
}

func (c *stubCatalog) ExtensionRoots() []string {
	return c.roots
}

// F13: with no relay service, the extension list, extension contributions and
// MCP server catalog must all still be served from the injected catalog.
func TestExtensionCatalogServedWithoutRelay(t *testing.T) {
	catalog := &stubCatalog{
		items: []extensions.ListedExtension{
			{ManifestPath: "/x/plugin.json", Root: "/x", Status: "ok"},
		},
		roots: []string{"/x"},
		contrib: extensions.EffectiveContributions{
			MCPServers: []extensions.EffectiveMCPServerContribution{
				{},
			},
		},
	}
	// NewServerWithCatalog leaves relay nil on purpose.
	srv := NewServerWithCatalog(&MockAPI{}, catalog)
	if srv.relay != nil {
		t.Fatal("relay must stay nil in this test")
	}

	// /api/extensions must be routed (registered) and return the catalog data.
	for _, path := range []string{"/api/extensions", "/api/extensions/contributions", "/api/mcp/servers"} {
		req := httptest.NewRequest(http.MethodGet, path, nil)
		rec := httptest.NewRecorder()
		srv.ServeHTTP(rec, req)
		if rec.Code != http.StatusOK {
			t.Fatalf("%s: expected 200 without relay, got %d: %s", path, rec.Code, rec.Body.String())
		}
	}

	// /api/extensions returns the stubbed extension list.
	req := httptest.NewRequest(http.MethodGet, "/api/extensions", nil)
	rec := httptest.NewRecorder()
	srv.ServeHTTP(rec, req)
	var listResp struct {
		OK     bool `json:"ok"`
		Count  int  `json:"count"`
		Roots  []string `json:"roots"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &listResp); err != nil {
		t.Fatalf("decode extensions: %v", err)
	}
	if !listResp.OK || listResp.Count != 1 || len(listResp.Roots) != 1 {
		t.Fatalf("unexpected extensions payload: %s", rec.Body.String())
	}
}
