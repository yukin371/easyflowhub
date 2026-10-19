package relay

import (
	"encoding/json"
	"net/http"

	"scriptmgr/internal/localapi"
)

type Server struct {
	service *Service
	mux     *http.ServeMux
	handler http.Handler
}

func NewServer(stateDir string) (*Server, error) {
	service, err := NewService(stateDir)
	if err != nil {
		return nil, err
	}
	return NewServerWithService(service), nil
}

func NewServerWithService(service *Service) *Server {
	s := &Server{
		service: service,
		mux:     http.NewServeMux(),
	}
	s.routes()
	s.handler = localapi.WithBrowserOrigins(s.mux)
	return s
}

func (s *Server) routes() {
	s.mux.HandleFunc("GET /health", s.handleHealth)
	s.mux.HandleFunc("GET /api/relay/config", s.handleGetConfig)
	s.mux.HandleFunc("PUT /api/relay/config", s.handlePutConfig)
	s.mux.HandleFunc("GET /api/extensions", s.handleListExtensions)
	s.mux.HandleFunc("GET /api/extensions/contributions", s.handleEffectiveContributions)
	s.mux.Handle("/v1/", s.service.ProxyHandler(""))
	s.mux.Handle("/relay/v1/", s.service.ProxyHandler("/relay"))
}

func (s *Server) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	s.handler.ServeHTTP(w, r)
}

func (s *Server) handleHealth(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]any{
		"ok":      true,
		"service": "relay",
	})
}

func (s *Server) handleGetConfig(w http.ResponseWriter, r *http.Request) {
	snapshot, err := s.service.Snapshot()
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]any{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}

	writeJSON(w, http.StatusOK, map[string]any{
		"ok":       true,
		"snapshot": snapshot,
	})
}

func (s *Server) handlePutConfig(w http.ResponseWriter, r *http.Request) {
	var cfg Config
	if err := json.NewDecoder(r.Body).Decode(&cfg); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]any{
			"ok":    false,
			"error": "invalid JSON: " + err.Error(),
		})
		return
	}

	if err := s.service.SaveConfig(cfg); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]any{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}

	snapshot, err := s.service.Snapshot()
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]any{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}

	writeJSON(w, http.StatusOK, map[string]any{
		"ok":       true,
		"snapshot": snapshot,
	})
}

func (s *Server) handleListExtensions(w http.ResponseWriter, r *http.Request) {
	items, roots, err := s.service.ListExtensionsForAPI()
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]any{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}

	writeJSON(w, http.StatusOK, map[string]any{
		"ok":         true,
		"roots":      roots,
		"count":      len(items),
		"extensions": items,
	})
}

func (s *Server) handleEffectiveContributions(w http.ResponseWriter, r *http.Request) {
	contributions, err := s.service.EffectiveContributionsForAPI()
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]any{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}

	writeJSON(w, http.StatusOK, map[string]any{
		"ok":            true,
		"roots":         s.service.ExtensionRoots(),
		"contributions": contributions,
	})
}
