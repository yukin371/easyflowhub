package relay

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"scriptmgr/internal/extensions"
)

func TestStoreLoadSaveRoundTrip(t *testing.T) {
	store := NewStore(t.TempDir())
	cfg := Config{
		Version: 1,
		Providers: []Provider{
			{
				ID:      "primary",
				Name:    "Primary",
				BaseURL: "https://example.com",
				Enabled: true,
				Weight:  2,
			},
		},
		Routes: []Route{
			{
				ID:           "chat",
				PathPrefixes: []string{"/v1/"},
				ProviderIDs:  []string{"primary"},
			},
		},
	}

	if err := store.Save(cfg); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	loaded, err := store.Load()
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}

	if loaded.Providers[0].Weight != 2 {
		t.Fatalf("expected weight 2, got %d", loaded.Providers[0].Weight)
	}
	if loaded.Routes[0].Strategy != DefaultStrategy {
		t.Fatalf("expected default strategy %q, got %q", DefaultStrategy, loaded.Routes[0].Strategy)
	}
}

func TestProxyRetriesSecondProviderOnFailure(t *testing.T) {
	failServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.Error(w, "upstream failed", http.StatusBadGateway)
	}))
	defer failServer.Close()

	successServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if got := r.Header.Get("X-EasyFlowHub-Relay-Provider"); got != "secondary" {
			t.Fatalf("expected provider header to be secondary, got %q", got)
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"ok":true}`))
	}))
	defer successServer.Close()

	store := NewStore(t.TempDir())
	if err := store.Save(Config{
		Version: 1,
		Providers: []Provider{
			{ID: "primary", Name: "Primary", BaseURL: failServer.URL, Enabled: true},
			{ID: "secondary", Name: "Secondary", BaseURL: successServer.URL, Enabled: true},
		},
		Routes: []Route{
			{
				ID:            "openai",
				PathPrefixes:  []string{"/v1/"},
				ModelPatterns: []string{"gpt-*"},
				ProviderIDs:   []string{"primary", "secondary"},
			},
		},
	}); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	service := NewServiceWithDeps(store, extensions.NewRegistryWithRoots(nil))
	body := []byte(`{"model":"gpt-5","messages":[]}`)
	req := httptest.NewRequest(http.MethodPost, "/v1/chat/completions", bytes.NewReader(body))
	rec := httptest.NewRecorder()

	service.ProxyHandler("").ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rec.Code, rec.Body.String())
	}
	if provider := rec.Header().Get("X-EasyFlowHub-Relay-Provider"); provider != "secondary" {
		t.Fatalf("expected secondary to answer, got %q", provider)
	}
}

func TestServerListsExtensions(t *testing.T) {
	stateDir := t.TempDir()
	root := filepath.Join(stateDir, "extensions", "sample")
	if err := os.MkdirAll(root, 0o755); err != nil {
		t.Fatalf("MkdirAll failed: %v", err)
	}
	content := `{"id":"sample","name":"Sample","version":"1.0.0"}`
	if err := os.WriteFile(filepath.Join(root, "plugin.json"), []byte(content), 0o644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	server, err := NewServer(stateDir)
	if err != nil {
		t.Fatalf("NewServer failed: %v", err)
	}

	req := httptest.NewRequest(http.MethodGet, "/api/extensions", nil)
	rec := httptest.NewRecorder()
	server.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", rec.Code)
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
	if err := json.NewDecoder(strings.NewReader(rec.Body.String())).Decode(&response); err != nil {
		t.Fatalf("Decode failed: %v", err)
	}
	if !response.OK || response.Count != 1 {
		t.Fatalf("unexpected response: %+v", response)
	}
	if response.Extensions[0].Manifest == nil || response.Extensions[0].Manifest.ID != "sample" {
		t.Fatalf("expected sample manifest, got %+v", response.Extensions[0].Manifest)
	}
}
