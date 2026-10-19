package localapi

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestLoopbackAddress(t *testing.T) {
	if got := LoopbackAddress("8787"); got != "127.0.0.1:8787" {
		t.Fatalf("expected loopback address, got %q", got)
	}
}

func TestWithBrowserOriginsAllowsDesktopAndDevelopmentOrigins(t *testing.T) {
	for _, origin := range []string{
		"http://localhost:1420",
		"http://127.0.0.1:1420",
		"http://tauri.localhost",
		"tauri://localhost",
	} {
		t.Run(origin, func(t *testing.T) {
			handler := WithBrowserOrigins(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
				w.WriteHeader(http.StatusOK)
			}))
			req := httptest.NewRequest(http.MethodGet, "/health", nil)
			req.Header.Set("Origin", origin)
			rec := httptest.NewRecorder()

			handler.ServeHTTP(rec, req)

			if rec.Code != http.StatusOK {
				t.Fatalf("expected 200, got %d", rec.Code)
			}
			if got := rec.Header().Get("Access-Control-Allow-Origin"); got != origin {
				t.Fatalf("expected exact allowed origin %q, got %q", origin, got)
			}
			if got := rec.Header().Get("Vary"); got != "Origin" {
				t.Fatalf("expected Vary: Origin, got %q", got)
			}
		})
	}
}

func TestWithBrowserOriginsHandlesPreflight(t *testing.T) {
	handler := WithBrowserOrigins(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		t.Fatal("preflight must not reach the wrapped handler")
	}))
	req := httptest.NewRequest(http.MethodOptions, "/api/relay/config", nil)
	req.Header.Set("Origin", "http://tauri.localhost")
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusNoContent {
		t.Fatalf("expected 204, got %d", rec.Code)
	}
	if got := rec.Header().Get("Access-Control-Allow-Methods"); got == "" {
		t.Fatal("expected allowed methods header")
	}
	if got := rec.Header().Get("Access-Control-Allow-Headers"); got != "Content-Type, Authorization" {
		t.Fatalf("unexpected allowed headers %q", got)
	}
}

func TestWithBrowserOriginsRejectsUntrustedOrigin(t *testing.T) {
	handler := WithBrowserOrigins(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		t.Fatal("untrusted origin must not reach the wrapped handler")
	}))
	req := httptest.NewRequest(http.MethodGet, "/api/relay/config", nil)
	req.Header.Set("Origin", "https://attacker.example")
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusForbidden {
		t.Fatalf("expected 403, got %d", rec.Code)
	}
	if got := rec.Header().Get("Access-Control-Allow-Origin"); got != "" {
		t.Fatalf("unexpected CORS permission for untrusted origin %q", got)
	}
}

func TestWithBrowserOriginsPreservesOriginlessClients(t *testing.T) {
	handler := WithBrowserOrigins(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	req := httptest.NewRequest(http.MethodGet, "/health", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected originless client to pass, got %d", rec.Code)
	}
	if got := rec.Header().Get("Access-Control-Allow-Origin"); got != "" {
		t.Fatalf("did not expect CORS headers for originless client, got %q", got)
	}
}
