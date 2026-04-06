package relay

import (
	"bytes"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"scriptmgr/internal/extensions"
)

func newRelayServiceForTest(t *testing.T, cfg Config) *Service {
	t.Helper()

	store := NewStore(t.TempDir())
	if err := store.Save(cfg); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	return NewServiceWithDeps(store, extensions.NewRegistryWithRoots(nil))
}

func performRelayRequest(t *testing.T, service *Service, method, target string, body []byte, headers map[string]string) *httptest.ResponseRecorder {
	t.Helper()

	req := httptest.NewRequest(method, target, bytes.NewReader(body))
	for key, value := range headers {
		req.Header.Set(key, value)
	}
	rec := httptest.NewRecorder()
	service.ProxyHandler("").ServeHTTP(rec, req)
	return rec
}

func snapshotByProviderID(t *testing.T, service *Service) map[string]ProviderSnapshot {
	t.Helper()

	snapshot, err := service.Snapshot()
	if err != nil {
		t.Fatalf("Snapshot failed: %v", err)
	}

	items := make(map[string]ProviderSnapshot, len(snapshot.Providers))
	for _, item := range snapshot.Providers {
		items[item.Provider.ID] = item
	}
	return items
}

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

	service := newRelayServiceForTest(t, Config{
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
	})
	body := []byte(`{"model":"gpt-5","messages":[]}`)
	rec := performRelayRequest(t, service, http.MethodPost, "/v1/chat/completions", body, nil)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rec.Code, rec.Body.String())
	}
	if provider := rec.Header().Get("X-EasyFlowHub-Relay-Provider"); provider != "secondary" {
		t.Fatalf("expected secondary to answer, got %q", provider)
	}
}

func TestProxyPreservesStreamingResponse(t *testing.T) {
	streamServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/v1/responses" {
			t.Fatalf("expected /v1/responses, got %q", r.URL.Path)
		}

		w.Header().Set("Content-Type", "text/event-stream")
		w.Header().Set("Cache-Control", "no-cache")
		flusher, ok := w.(http.Flusher)
		if !ok {
			t.Fatal("expected flusher")
		}

		_, _ = io.WriteString(w, "data: first\n\n")
		flusher.Flush()
		_, _ = io.WriteString(w, "data: second\n\n")
	}))
	defer streamServer.Close()

	service := newRelayServiceForTest(t, Config{
		Version: 1,
		Providers: []Provider{
			{ID: "streamer", Name: "Streamer", BaseURL: streamServer.URL, Enabled: true},
		},
		Routes: []Route{
			{
				ID:           "stream",
				PathPrefixes: []string{"/v1/"},
				ProviderIDs:  []string{"streamer"},
			},
		},
	})

	rec := performRelayRequest(
		t,
		service,
		http.MethodPost,
		"/v1/responses",
		[]byte(`{"model":"gpt-5","stream":true}`),
		map[string]string{"Accept": "text/event-stream"},
	)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rec.Code, rec.Body.String())
	}
	if got := rec.Header().Get("Content-Type"); got != "text/event-stream" {
		t.Fatalf("expected event-stream content type, got %q", got)
	}
	if got := rec.Body.String(); got != "data: first\n\ndata: second\n\n" {
		t.Fatalf("unexpected stream body %q", got)
	}
	if provider := rec.Header().Get("X-EasyFlowHub-Relay-Provider"); provider != "streamer" {
		t.Fatalf("expected streamer to answer, got %q", provider)
	}
}

func TestProxyRetriesOn429AndUsesProviderSpecificAuth(t *testing.T) {
	primaryServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if got := r.Header.Get("Authorization"); got != "Bearer primary-secret" {
			t.Fatalf("expected primary auth header, got %q", got)
		}
		http.Error(w, "rate limited", http.StatusTooManyRequests)
	}))
	defer primaryServer.Close()

	secondaryServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if got := r.Header.Get("Authorization"); got != "Token secondary-secret" {
			t.Fatalf("expected secondary authorization override, got %q", got)
		}
		if got := r.Header.Get("X-Provider-Key"); got != "secondary-secret" {
			t.Fatalf("expected secondary provider key header, got %q", got)
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"ok":true,"provider":"secondary"}`))
	}))
	defer secondaryServer.Close()

	service := newRelayServiceForTest(t, Config{
		Version: 1,
		Providers: []Provider{
			{
				ID:      "primary",
				Name:    "Primary",
				BaseURL: primaryServer.URL,
				Enabled: true,
				APIKey:  "primary-secret",
			},
			{
				ID:      "secondary",
				Name:    "Secondary",
				BaseURL: secondaryServer.URL,
				Enabled: true,
				Headers: map[string]string{
					"Authorization": "Token secondary-secret",
					"X-Provider-Key": "secondary-secret",
				},
			},
		},
		Routes: []Route{
			{
				ID:            "chat",
				PathPrefixes:  []string{"/v1/"},
				ModelPatterns: []string{"gpt-*"},
				ProviderIDs:   []string{"primary", "secondary"},
			},
		},
	})

	rec := performRelayRequest(
		t,
		service,
		http.MethodPost,
		"/v1/chat/completions",
		[]byte(`{"model":"gpt-5","messages":[]}`),
		map[string]string{"Authorization": "Bearer caller-token"},
	)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rec.Code, rec.Body.String())
	}
	if provider := rec.Header().Get("X-EasyFlowHub-Relay-Provider"); provider != "secondary" {
		t.Fatalf("expected secondary to answer, got %q", provider)
	}

	snapshots := snapshotByProviderID(t, service)
	if got := snapshots["primary"].Status.LastStatusCode; got != http.StatusTooManyRequests {
		t.Fatalf("expected primary last status 429, got %d", got)
	}
	if got := snapshots["primary"].Status.LastError; !strings.Contains(got, "429") {
		t.Fatalf("expected primary error to mention 429, got %q", got)
	}
}

func TestProxyRetriesOnTimeout(t *testing.T) {
	timeoutServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		time.Sleep(150 * time.Millisecond)
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte(`{"ok":true,"provider":"slow"}`))
	}))
	defer timeoutServer.Close()

	fastServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"ok":true,"provider":"fast"}`))
	}))
	defer fastServer.Close()

	service := newRelayServiceForTest(t, Config{
		Version: 1,
		Providers: []Provider{
			{
				ID:        "slow",
				Name:      "Slow",
				BaseURL:   timeoutServer.URL,
				Enabled:   true,
				TimeoutMs: 50,
			},
			{
				ID:      "fast",
				Name:    "Fast",
				BaseURL: fastServer.URL,
				Enabled: true,
			},
		},
		Routes: []Route{
			{
				ID:            "chat",
				PathPrefixes:  []string{"/v1/"},
				ModelPatterns: []string{"gpt-*"},
				ProviderIDs:   []string{"slow", "fast"},
			},
		},
	})

	rec := performRelayRequest(
		t,
		service,
		http.MethodPost,
		"/v1/chat/completions",
		[]byte(`{"model":"gpt-5","messages":[]}`),
		nil,
	)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rec.Code, rec.Body.String())
	}
	if provider := rec.Header().Get("X-EasyFlowHub-Relay-Provider"); provider != "fast" {
		t.Fatalf("expected fast to answer, got %q", provider)
	}

	snapshots := snapshotByProviderID(t, service)
	if got := snapshots["slow"].Status.ConsecutiveFailures; got != 1 {
		t.Fatalf("expected slow provider to record one failure, got %d", got)
	}
	if got := snapshots["slow"].Status.LastError; got == "" {
		t.Fatal("expected slow provider to record timeout error")
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
