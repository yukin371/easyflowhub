package relay

import (
	"bytes"
	"encoding/json"
	"fmt"
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

func writeManifest(t *testing.T, path string, content string) {
	t.Helper()

	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatalf("MkdirAll failed: %v", err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}
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

func TestSnapshotRedactsSecretsAndSaveConfigPreservesOrClearsThem(t *testing.T) {
	service := newRelayServiceForTest(t, Config{
		Version: 1,
		Providers: []Provider{{
			ID:      "primary",
			Name:    "Primary",
			BaseURL: "https://user:base-url-secret@example.com/v1?tenant=query-secret",
			APIKey:  "plain-secret",
			Headers: map[string]string{
				"Authorization":      "Bearer header-secret",
				"X-API-Key":          "header-api-secret",
				"X-Session-ID":       "session-secret",
				"X-Signature":        "signature-secret",
				"X-Client-Assertion": "assertion-secret",
				"DPoP":               "dpop-secret",
				"X-Region":           "us-west",
			},
			Enabled: true,
		}},
	})

	snapshot, err := service.Snapshot()
	if err != nil {
		t.Fatalf("Snapshot failed: %v", err)
	}
	if got := snapshot.Config.Providers[0].APIKey; got != redactedSecretValue {
		t.Fatalf("expected config API key redaction, got %q", got)
	}
	if got := snapshot.EffectiveConfig.Providers[0].APIKey; got != redactedSecretValue {
		t.Fatalf("expected effective config API key redaction, got %q", got)
	}
	if got := snapshot.Providers[0].Provider.APIKey; got != redactedSecretValue {
		t.Fatalf("expected provider snapshot API key redaction, got %q", got)
	}
	for _, provider := range []Provider{snapshot.Config.Providers[0], snapshot.EffectiveConfig.Providers[0], snapshot.Providers[0].Provider} {
		if provider.BaseURL != redactedSecretValue {
			t.Fatalf("expected credential-bearing base_url to be redacted, got %q", provider.BaseURL)
		}
	}
	for _, provider := range []Provider{snapshot.Config.Providers[0], snapshot.EffectiveConfig.Providers[0], snapshot.Providers[0].Provider} {
		for _, name := range []string{"Authorization", "X-API-Key", "X-Session-ID", "X-Signature", "X-Client-Assertion", "DPoP"} {
			if provider.Headers[name] != redactedSecretValue {
				t.Fatalf("expected sensitive header %q redaction, got %#v", name, provider.Headers)
			}
		}
		if provider.Headers["X-Region"] != "us-west" {
			t.Fatalf("expected non-sensitive header to remain visible, got %#v", provider.Headers)
		}
	}
	encodedSnapshot, err := json.Marshal(snapshot)
	if err != nil {
		t.Fatalf("marshal snapshot failed: %v", err)
	}
	for _, secret := range []string{"plain-secret", "header-secret", "header-api-secret", "base-url-secret", "query-secret", "session-secret", "signature-secret", "assertion-secret", "dpop-secret"} {
		if strings.Contains(string(encodedSnapshot), secret) {
			t.Fatalf("snapshot leaked secret %q", secret)
		}
	}

	if err := service.SaveConfig(snapshot.Config); err != nil {
		t.Fatalf("saving unmodified redacted config failed: %v", err)
	}
	saved, err := service.LoadConfig()
	if err != nil {
		t.Fatalf("LoadConfig failed: %v", err)
	}
	if got := saved.Providers[0].APIKey; got != "plain-secret" {
		t.Fatalf("expected unchanged API key to be preserved, got %q", got)
	}
	if got := saved.Providers[0].BaseURL; got != "https://user:base-url-secret@example.com/v1?tenant=query-secret" {
		t.Fatalf("expected unchanged credential-bearing base_url to be preserved, got %q", got)
	}
	if got := saved.Providers[0].Headers["Authorization"]; got != "Bearer header-secret" {
		t.Fatalf("expected unchanged Authorization header to be preserved, got %q", got)
	}
	if got := saved.Providers[0].Headers["X-API-Key"]; got != "header-api-secret" {
		t.Fatalf("expected unchanged API key header to be preserved, got %q", got)
	}

	cleared := snapshot.Config
	cleared.Providers[0].APIKey = ""
	delete(cleared.Providers[0].Headers, "Authorization")
	cleared.Providers[0].Headers["X-API-Key"] = ""
	if err := service.SaveConfig(cleared); err != nil {
		t.Fatalf("saving cleared secrets failed: %v", err)
	}
	saved, err = service.LoadConfig()
	if err != nil {
		t.Fatalf("LoadConfig after clear failed: %v", err)
	}
	if saved.Providers[0].APIKey != "" || saved.Providers[0].Headers["Authorization"] != "" || saved.Providers[0].Headers["X-API-Key"] != "" {
		t.Fatalf("expected cleared secrets to be removed, got %#v", saved.Providers[0])
	}
	if saved.Providers[0].Headers["X-Region"] != "us-west" {
		t.Fatalf("expected non-sensitive header to remain, got %#v", saved.Providers[0].Headers)
	}
	if err := service.SaveConfig(Config{Version: 1, Providers: []Provider{{
		ID: "new", Name: "New", BaseURL: "https://new.example.com", APIKey: redactedSecretValue,
		Headers: map[string]string{"X-API-Key": redactedSecretValue}, Enabled: true,
	}}}); err != nil {
		t.Fatalf("saving new provider with placeholder failed: %v", err)
	}
	saved, err = service.LoadConfig()
	if err != nil {
		t.Fatalf("LoadConfig after new provider save failed: %v", err)
	}
	if saved.Providers[0].APIKey != "" || len(saved.Providers[0].Headers) != 0 {
		t.Fatalf("redaction marker must not be persisted as a credential: %#v", saved.Providers[0])
	}
}

func TestRelayServerConfigAPIUsesRedactedSnapshotWithoutLosingSecrets(t *testing.T) {
	stateDir := t.TempDir()
	writeManifest(t, filepath.Join(stateDir, "extensions", "manifest-provider", "plugin.json"), `{
  "id": "manifest-provider",
  "name": "Manifest Provider",
  "version": "1.0.0",
  "contributions": {
    "relay_providers": [{
      "id": "manifest-provider",
      "name": "Manifest Provider",
      "base_url": "https://extension-user:manifest-url-secret@extension.example.com?token=manifest-query-secret",
      "headers": {"X-Provider-Token": "manifest-header-secret"}
    }]
  }
}`)
	service, err := NewService(stateDir)
	if err != nil {
		t.Fatalf("NewService failed: %v", err)
	}
	if err := service.SaveConfig(Config{
		Version: 1,
		Providers: []Provider{{
			ID: "primary", Name: "Primary", BaseURL: "https://example.com", Enabled: true,
			APIKey: "relay-api-secret", Headers: map[string]string{"X-Provider-Token": "relay-header-secret"},
		}},
	}); err != nil {
		t.Fatalf("SaveConfig failed: %v", err)
	}
	server := NewServerWithService(service)

	get := httptest.NewRequest(http.MethodGet, "/api/relay/config", nil)
	getRec := httptest.NewRecorder()
	server.ServeHTTP(getRec, get)
	if getRec.Code != http.StatusOK {
		t.Fatalf("expected config GET 200, got %d: %s", getRec.Code, getRec.Body.String())
	}
	for _, secret := range []string{"relay-api-secret", "relay-header-secret", "manifest-header-secret", "manifest-url-secret", "manifest-query-secret"} {
		if strings.Contains(getRec.Body.String(), secret) {
			t.Fatalf("Relay config GET leaked secret %q", secret)
		}
	}
	var getResponse struct {
		Snapshot Snapshot `json:"snapshot"`
	}
	if err := json.Unmarshal(getRec.Body.Bytes(), &getResponse); err != nil {
		t.Fatalf("decode config GET failed: %v", err)
	}
	if getResponse.Snapshot.Config.Providers[0].APIKey != redactedSecretValue {
		t.Fatalf("expected redacted API key, got %q", getResponse.Snapshot.Config.Providers[0].APIKey)
	}
	if getResponse.Snapshot.EffectiveConfig.Providers[1].BaseURL != redactedSecretValue {
		t.Fatalf("expected extension URL credentials to be redacted, got %q", getResponse.Snapshot.EffectiveConfig.Providers[1].BaseURL)
	}
	if len(getResponse.Snapshot.Extensions) != 1 || getResponse.Snapshot.Extensions[0].Manifest.Contributions.RelayProviders[0].Headers["X-Provider-Token"] != redactedSecretValue {
		t.Fatalf("expected nested extension headers to be redacted, got %+v", getResponse.Snapshot.Extensions)
	}
	if getResponse.Snapshot.Extensions[0].Manifest.Contributions.RelayProviders[0].BaseURL != redactedSecretValue {
		t.Fatalf("expected nested extension URL credentials to be redacted, got %+v", getResponse.Snapshot.Extensions[0].Manifest.Contributions.RelayProviders[0])
	}

	for _, path := range []string{"/api/extensions", "/api/extensions/contributions"} {
		req := httptest.NewRequest(http.MethodGet, path, nil)
		rec := httptest.NewRecorder()
		server.ServeHTTP(rec, req)
		if rec.Code != http.StatusOK {
			t.Fatalf("expected %s GET 200, got %d: %s", path, rec.Code, rec.Body.String())
		}
		for _, secret := range []string{"manifest-header-secret", "manifest-url-secret", "manifest-query-secret"} {
			if strings.Contains(rec.Body.String(), secret) {
				t.Fatalf("%s leaked extension secret %q: %s", path, secret, rec.Body.String())
			}
		}
		if !strings.Contains(rec.Body.String(), redactedSecretValue) {
			t.Fatalf("%s did not redact extension header: %s", path, rec.Body.String())
		}
	}

	body, err := json.Marshal(getResponse.Snapshot.Config)
	if err != nil {
		t.Fatalf("marshal redacted config failed: %v", err)
	}
	put := httptest.NewRequest(http.MethodPut, "/api/relay/config", bytes.NewReader(body))
	putRec := httptest.NewRecorder()
	server.ServeHTTP(putRec, put)
	if putRec.Code != http.StatusOK {
		t.Fatalf("expected config PUT 200, got %d: %s", putRec.Code, putRec.Body.String())
	}
	for _, secret := range []string{"relay-api-secret", "relay-header-secret", "manifest-header-secret", "manifest-url-secret", "manifest-query-secret"} {
		if strings.Contains(putRec.Body.String(), secret) {
			t.Fatalf("Relay config PUT leaked secret %q", secret)
		}
	}
	saved, err := service.LoadConfig()
	if err != nil {
		t.Fatalf("LoadConfig after PUT failed: %v", err)
	}
	if saved.Providers[0].APIKey != "relay-api-secret" || saved.Providers[0].Headers["X-Provider-Token"] != "relay-header-secret" {
		t.Fatalf("redacted config PUT lost stored secrets: %#v", saved.Providers[0])
	}
}

func TestConfigValidateRejectsPlainAndEnvAPIKeyTogether(t *testing.T) {
	cfg := Config{
		Version: 1,
		Providers: []Provider{
			{
				ID:        "primary",
				Name:      "Primary",
				BaseURL:   "https://example.com",
				Enabled:   true,
				APIKey:    "plain-secret",
				APIKeyEnv: "PRIMARY_API_KEY",
			},
		},
	}

	if err := cfg.Normalize().Validate(); err == nil || !strings.Contains(err.Error(), "api_key_env") {
		t.Fatalf("expected api_key/api_key_env validation error, got %v", err)
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

func TestServerAcceptsRelayV1AliasForOpenAIChatCompletions(t *testing.T) {
	upstream := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/v1/chat/completions" {
			t.Fatalf("expected upstream OpenAI path /v1/chat/completions, got %q", r.URL.Path)
		}
		if r.Method != http.MethodPost {
			t.Fatalf("expected POST, got %s", r.Method)
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"choices":[{"message":{"content":"translated"}}]}`))
	}))
	defer upstream.Close()

	service := newRelayServiceForTest(t, Config{
		Version: 1,
		Providers: []Provider{{
			ID:      "translation",
			Name:    "Translation",
			BaseURL: upstream.URL,
			Enabled: true,
		}},
	})
	server := httptest.NewServer(NewServerWithService(service))
	defer server.Close()

	request, err := http.NewRequest(
		http.MethodPost,
		server.URL+"/relay/v1/chat/completions",
		strings.NewReader(`{"model":"translation-model","stream":false,"messages":[]}`),
	)
	if err != nil {
		t.Fatalf("build relay request failed: %v", err)
	}
	request.Header.Set("Content-Type", "application/json")
	request.Header.Set("Origin", "http://tauri.localhost")
	response, err := http.DefaultClient.Do(request)
	if err != nil {
		t.Fatalf("relay request failed: %v", err)
	}
	defer response.Body.Close()

	if response.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(response.Body)
		t.Fatalf("expected 200, got %d: %s", response.StatusCode, body)
	}
	if got := response.Header.Get("X-EasyFlowHub-Relay-Provider"); got != "translation" {
		t.Fatalf("expected translation provider header, got %q", got)
	}
	if got := response.Header.Get("Access-Control-Allow-Origin"); got != "http://tauri.localhost" {
		t.Fatalf("expected exact allowed CORS origin for browser preview, got %q", got)
	}
	body, err := io.ReadAll(response.Body)
	if err != nil {
		t.Fatalf("read response failed: %v", err)
	}
	if !strings.Contains(string(body), "translated") {
		t.Fatalf("expected translated response, got %s", body)
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
					"Authorization":  "Token secondary-secret",
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

func TestProxyClassifiesExhaustedProviderFailures(t *testing.T) {
	for _, tc := range []struct {
		name     string
		statuses []int
		want     int
	}{
		{name: "single rate limit", statuses: []int{429}, want: 429},
		{name: "all rate limited", statuses: []int{429, 429, 429}, want: 429},
		{name: "all unauthorized", statuses: []int{401, 401}, want: 401},
		{name: "all forbidden", statuses: []int{403, 403}, want: 403},
		{name: "mixed auth ends forbidden", statuses: []int{401, 403}, want: 403},
		{name: "mixed auth ends unauthorized", statuses: []int{403, 401}, want: 401},
		{name: "rate limit and server failure", statuses: []int{429, 503}, want: 502},
		{name: "server failure and rate limit", statuses: []int{503, 429}, want: 502},
		{name: "auth and rate limit", statuses: []int{401, 429}, want: 502},
		{name: "auth and server failure", statuses: []int{403, 500}, want: 502},
		{name: "network and rate limit", statuses: []int{0, 429}, want: 502},
		{name: "auth and network", statuses: []int{401, 0}, want: 502},
	} {
		t.Run(tc.name, func(t *testing.T) {
			var providers []Provider
			for index, status := range tc.statuses {
				upstream := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
					w.Header().Set("X-Upstream-Secret", "SECRET")
					http.Error(w, "SECRET provider credentials and diagnostics", status)
				}))
				t.Cleanup(upstream.Close)
				if status == 0 {
					// Keep a real closed address to exercise a transport failure,
					// rather than treating an HTTP status as a network failure.
					upstream.Close()
				}
				id := fmt.Sprintf("provider-%d", index)
				providers = append(providers, Provider{ID: id, Name: id, BaseURL: upstream.URL, Enabled: true, APIKey: "SECRET"})
			}
			service := newRelayServiceForTest(t, Config{Version: 1, Providers: providers})
			rec := performRelayRequest(t, service, http.MethodPost, "/v1/chat/completions", []byte(`{"model":"test-model","messages":[]}`), nil)
			if rec.Code != tc.want {
				t.Fatalf("expected HTTP %d, got %d: %s", tc.want, rec.Code, rec.Body.String())
			}
			var response struct {
				Error     string   `json:"error"`
				Attempted []string `json:"attempted"`
			}
			if err := json.Unmarshal(rec.Body.Bytes(), &response); err != nil {
				t.Fatalf("invalid exhaustion response: %v", err)
			}
			if response.Error != "all upstream providers failed" || len(response.Attempted) != len(providers) {
				t.Fatalf("unexpected exhaustion response: %+v", response)
			}
			if strings.Contains(rec.Body.String(), "SECRET") || rec.Header().Get("X-Upstream-Secret") != "" {
				t.Fatal("exhaustion response leaked upstream content or headers")
			}
			snapshots := snapshotByProviderID(t, service)
			for index, status := range tc.statuses {
				provider := snapshots[providers[index].ID]
				if provider.Status.LastPickedAt == nil || provider.Status.LastStatusCode != status {
					t.Fatalf("provider %d was not attempted or recorded correctly", index)
				}
			}
		})
	}
}

func TestProxyRetriesAfterAuthenticationFailure(t *testing.T) {
	for _, status := range []int{http.StatusUnauthorized, http.StatusForbidden} {
		t.Run(http.StatusText(status), func(t *testing.T) {
			primary := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				http.Error(w, "SECRET rejected key", status)
			}))
			defer primary.Close()
			secondary := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				w.Header().Set("Content-Type", "application/json")
				_, _ = w.Write([]byte(`{"choices":[{"message":{"content":"translated"}}]}`))
			}))
			defer secondary.Close()
			service := newRelayServiceForTest(t, Config{
				Version: 1,
				Providers: []Provider{
					{ID: "primary", Name: "Primary", BaseURL: primary.URL, Enabled: true},
					{ID: "secondary", Name: "Secondary", BaseURL: secondary.URL, Enabled: true},
				},
			})
			rec := performRelayRequest(t, service, http.MethodPost, "/v1/chat/completions", []byte(`{"model":"test-model","messages":[]}`), nil)
			if rec.Code != http.StatusOK || rec.Header().Get("X-EasyFlowHub-Relay-Provider") != "secondary" || !strings.Contains(rec.Body.String(), "translated") {
				t.Fatalf("expected secondary success, got %d: %s", rec.Code, rec.Body.String())
			}
			if got := snapshotByProviderID(t, service)["primary"].Status.LastStatusCode; got != status {
				t.Fatalf("expected primary failure status %d, got %d", status, got)
			}
		})
	}
}

func TestProxyNoMatchingProviderRemainsGatewayError(t *testing.T) {
	service := newRelayServiceForTest(t, Config{
		Version: 1,
		Providers: []Provider{{
			ID: "primary", Name: "Primary", BaseURL: "http://127.0.0.1:1", Enabled: true, ModelPatterns: []string{"other-model"},
		}},
	})
	rec := performRelayRequest(t, service, http.MethodPost, "/v1/chat/completions", []byte(`{"model":"test-model","messages":[]}`), nil)
	if rec.Code != http.StatusBadGateway || !strings.Contains(rec.Body.String(), "all upstream providers failed") {
		t.Fatalf("expected generic gateway error for no matching candidates, got %d: %s", rec.Code, rec.Body.String())
	}
}

func TestProxyNoConfiguredProviderHasStableError(t *testing.T) {
	service := newRelayServiceForTest(t, Config{Version: 1})
	rec := performRelayRequest(t, service, http.MethodPost, "/v1/chat/completions", []byte(`{"model":"test-model","messages":[]}`), nil)
	if rec.Code != http.StatusBadGateway || !strings.Contains(rec.Body.String(), "no enabled relay providers configured") {
		t.Fatalf("expected stable no-provider error, got %d: %s", rec.Code, rec.Body.String())
	}
}

func TestTransportFailureSnapshotDoesNotExposeRequestURLCredentials(t *testing.T) {
	service := newRelayServiceForTest(t, Config{
		Version: 1,
		Providers: []Provider{{
			ID: "primary", Name: "Primary",
			BaseURL: "http://url-user:url-password@127.0.0.1:1/v1?token=base-query-secret",
			Enabled: true,
		}},
	})
	rec := performRelayRequest(
		t,
		service,
		http.MethodPost,
		"/v1/chat/completions?client_key=request-query-secret",
		[]byte(`{"model":"test-model","messages":[]}`),
		nil,
	)
	if rec.Code != http.StatusBadGateway {
		t.Fatalf("expected gateway error for unavailable upstream, got %d: %s", rec.Code, rec.Body.String())
	}
	status := snapshotByProviderID(t, service)["primary"].Status
	for _, secret := range []string{"url-user", "url-password", "base-query-secret", "request-query-secret"} {
		if strings.Contains(status.LastError, secret) {
			t.Fatalf("provider status leaked URL credential %q: %s", secret, status.LastError)
		}
	}
}

func TestProxyRetriesOn5xx(t *testing.T) {
	primaryServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.Error(w, "upstream unavailable", http.StatusServiceUnavailable)
	}))
	defer primaryServer.Close()

	secondaryServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"ok":true,"provider":"secondary"}`))
	}))
	defer secondaryServer.Close()

	service := newRelayServiceForTest(t, Config{
		Version: 1,
		Providers: []Provider{
			{ID: "primary", Name: "Primary", BaseURL: primaryServer.URL, Enabled: true},
			{ID: "secondary", Name: "Secondary", BaseURL: secondaryServer.URL, Enabled: true},
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
		nil,
	)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rec.Code, rec.Body.String())
	}
	if provider := rec.Header().Get("X-EasyFlowHub-Relay-Provider"); provider != "secondary" {
		t.Fatalf("expected secondary to answer, got %q", provider)
	}

	snapshots := snapshotByProviderID(t, service)
	if got := snapshots["primary"].Status.LastStatusCode; got != http.StatusServiceUnavailable {
		t.Fatalf("expected primary last status 503, got %d", got)
	}
	if got := snapshots["primary"].Status.LastError; !strings.Contains(got, "503") {
		t.Fatalf("expected primary error to mention 503, got %q", got)
	}
}

func TestProxyUsesAPIKeyFromEnvironment(t *testing.T) {
	t.Setenv("EASYFLOWHUB_RELAY_PRIMARY_KEY", "env-secret")

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if got := r.Header.Get("Authorization"); got != "Bearer env-secret" {
			t.Fatalf("expected env-based authorization, got %q", got)
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"ok":true}`))
	}))
	defer server.Close()

	service := newRelayServiceForTest(t, Config{
		Version: 1,
		Providers: []Provider{
			{
				ID:        "primary",
				Name:      "Primary",
				BaseURL:   server.URL,
				Enabled:   true,
				APIKeyEnv: "EASYFLOWHUB_RELAY_PRIMARY_KEY",
			},
		},
		Routes: []Route{
			{
				ID:           "chat",
				PathPrefixes: []string{"/v1/"},
				ProviderIDs:  []string{"primary"},
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
}

func TestProxyFallsBackWhenAPIKeyEnvMissing(t *testing.T) {
	primaryServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		t.Fatal("primary server should not receive request when api_key_env is missing")
	}))
	defer primaryServer.Close()

	secondaryServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"ok":true,"provider":"secondary"}`))
	}))
	defer secondaryServer.Close()

	service := newRelayServiceForTest(t, Config{
		Version: 1,
		Providers: []Provider{
			{
				ID:        "primary",
				Name:      "Primary",
				BaseURL:   primaryServer.URL,
				Enabled:   true,
				APIKeyEnv: "EASYFLOWHUB_RELAY_MISSING_KEY",
			},
			{
				ID:      "secondary",
				Name:    "Secondary",
				BaseURL: secondaryServer.URL,
				Enabled: true,
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
		nil,
	)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rec.Code, rec.Body.String())
	}
	if provider := rec.Header().Get("X-EasyFlowHub-Relay-Provider"); provider != "secondary" {
		t.Fatalf("expected secondary to answer, got %q", provider)
	}

	snapshots := snapshotByProviderID(t, service)
	if got := snapshots["primary"].Status.LastError; !strings.Contains(got, "missing env") {
		t.Fatalf("expected missing env error, got %q", got)
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

func TestEffectiveConfigMergesExtensionRelayContributions(t *testing.T) {
	stateDir := t.TempDir()
	store := NewStore(stateDir)
	if err := store.Save(DefaultConfig()); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	root := filepath.Join(stateDir, "extensions")
	writeManifest(t, filepath.Join(root, "sample", "plugin.json"), `{
  "id": "sample",
  "name": "Sample",
  "version": "1.0.0",
  "contributions": {
    "relay_providers": [
      { "id": "provider-ext", "name": "Extension Provider", "base_url": "https://ext.example.com" }
    ],
    "relay_routes": [
      { "id": "route-ext", "provider_ids": ["provider-ext"], "path_prefixes": ["/v1/"] }
    ]
  }
}`)

	service := NewServiceWithDeps(store, extensions.NewRegistryWithRoots([]string{root}))
	cfg, err := service.EffectiveConfig()
	if err != nil {
		t.Fatalf("EffectiveConfig failed: %v", err)
	}

	if len(cfg.Providers) != 1 || cfg.Providers[0].ID != "provider-ext" {
		t.Fatalf("unexpected providers %+v", cfg.Providers)
	}
	if cfg.Providers[0].Source != "extension:sample" {
		t.Fatalf("unexpected provider source %q", cfg.Providers[0].Source)
	}
	if len(cfg.Routes) != 1 || cfg.Routes[0].ID != "route-ext" {
		t.Fatalf("unexpected routes %+v", cfg.Routes)
	}
}

func TestProxyUsesExtensionRelayContributions(t *testing.T) {
	upstream := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"ok":true,"provider":"extension"}`))
	}))
	defer upstream.Close()

	stateDir := t.TempDir()
	store := NewStore(stateDir)
	if err := store.Save(DefaultConfig()); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	root := filepath.Join(stateDir, "extensions")
	writeManifest(t, filepath.Join(root, "sample", "plugin.json"), `{
  "id": "sample",
  "name": "Sample",
  "version": "1.0.0",
  "contributions": {
    "relay_providers": [
      { "id": "provider-ext", "name": "Extension Provider", "base_url": "`+upstream.URL+`" }
    ]
  }
}`)

	service := NewServiceWithDeps(store, extensions.NewRegistryWithRoots([]string{root}))
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
	if provider := rec.Header().Get("X-EasyFlowHub-Relay-Provider"); provider != "provider-ext" {
		t.Fatalf("expected extension provider, got %q", provider)
	}
}

func TestServerListsEffectiveContributions(t *testing.T) {
	stateDir := t.TempDir()
	root := filepath.Join(stateDir, "extensions")
	writeManifest(t, filepath.Join(root, "sample", "plugin.json"), `{
  "id": "sample",
  "name": "Sample",
  "version": "1.0.0",
  "contributions": {
    "relay_providers": [
      { "id": "provider-ext", "name": "Extension Provider", "base_url": "https://ext.example.com" }
    ]
  }
}`)

	server, err := NewServer(stateDir)
	if err != nil {
		t.Fatalf("NewServer failed: %v", err)
	}

	req := httptest.NewRequest(http.MethodGet, "/api/extensions/contributions", nil)
	rec := httptest.NewRecorder()
	server.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", rec.Code)
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
	if err := json.NewDecoder(strings.NewReader(rec.Body.String())).Decode(&response); err != nil {
		t.Fatalf("Decode failed: %v", err)
	}
	if !response.OK || len(response.Contributions.RelayProviders) != 1 {
		t.Fatalf("unexpected response %+v", response)
	}
	if response.Contributions.RelayProviders[0].Source.ExtensionID != "sample" {
		t.Fatalf("unexpected contribution source %+v", response.Contributions.RelayProviders[0].Source)
	}
}
