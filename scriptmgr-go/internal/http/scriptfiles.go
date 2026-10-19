package http

import (
	"encoding/json"
	"net/http"
	"strings"
	"time"

	"scriptmgr/internal/model"
)

// handleListRoots returns every discovered root plus the user-pinned subset.
func (s *Server) handleListRoots(w http.ResponseWriter, r *http.Request) {
	roots, custom, err := s.api.ListRootDetails()
	if err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}
	s.writeJSON(w, http.StatusOK, model.RootsDetailResponse{
		OK:          true,
		Roots:       roots,
		Custom:      custom,
		GeneratedAt: time.Now().UTC().Format(time.RFC3339),
	})
}

// handleAddRoot pins a directory as a stable script storage location.
func (s *Server) handleAddRoot(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Path string `json:"path"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		s.writeError(w, http.StatusBadRequest, "invalid JSON: "+err.Error())
		return
	}
	path := strings.TrimSpace(req.Path)
	if path == "" {
		s.writeError(w, http.StatusBadRequest, "path is required")
		return
	}

	cleaned, err := s.api.AddRoot(path)
	if err != nil {
		s.writeError(w, http.StatusBadRequest, err.Error())
		return
	}
	roots, custom, err := s.api.ListRootDetails()
	if err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}
	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":           true,
		"root":         cleaned,
		"roots":        roots,
		"custom":       custom,
		"generated_at": time.Now().UTC().Format(time.RFC3339),
	})
}

// handleRemoveRoot unpins a directory. Auto-discovered roots cannot be removed.
func (s *Server) handleRemoveRoot(w http.ResponseWriter, r *http.Request) {
	path := strings.TrimSpace(r.URL.Query().Get("path"))
	if path == "" {
		s.writeError(w, http.StatusBadRequest, "path query parameter is required")
		return
	}
	if err := s.api.RemoveRoot(path); err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}
	roots, custom, err := s.api.ListRootDetails()
	if err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}
	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":           true,
		"roots":        roots,
		"custom":       custom,
		"generated_at": time.Now().UTC().Format(time.RFC3339),
	})
}

func (s *Server) handleReadScriptSource(w http.ResponseWriter, r *http.Request) {
	scriptID := r.PathValue("id")
	if scriptID == "" {
		s.writeError(w, http.StatusBadRequest, "missing script id")
		return
	}
	source, err := s.api.ReadScriptSource(scriptID)
	if err != nil {
		s.writeError(w, http.StatusNotFound, err.Error())
		return
	}
	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":           true,
		"source":       source,
		"generated_at": time.Now().UTC().Format(time.RFC3339),
	})
}

func (s *Server) handleWriteScriptSource(w http.ResponseWriter, r *http.Request) {
	scriptID := r.PathValue("id")
	if scriptID == "" {
		s.writeError(w, http.StatusBadRequest, "missing script id")
		return
	}
	var req struct {
		Content string `json:"content"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		s.writeError(w, http.StatusBadRequest, "invalid JSON: "+err.Error())
		return
	}
	source, err := s.api.WriteScriptSource(scriptID, req.Content)
	if err != nil {
		s.writeError(w, http.StatusBadRequest, err.Error())
		return
	}
	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":           true,
		"source":       source,
		"generated_at": time.Now().UTC().Format(time.RFC3339),
	})
}

func (s *Server) handleCreateScript(w http.ResponseWriter, r *http.Request) {
	var req model.CreateScriptRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		s.writeError(w, http.StatusBadRequest, "invalid JSON: "+err.Error())
		return
	}
	script, err := s.api.CreateScript(req)
	if err != nil {
		s.writeError(w, http.StatusBadRequest, err.Error())
		return
	}
	s.writeJSON(w, http.StatusCreated, map[string]any{
		"ok":           true,
		"script":       script,
		"generated_at": time.Now().UTC().Format(time.RFC3339),
	})
}

func (s *Server) handleImportScripts(w http.ResponseWriter, r *http.Request) {
	var req model.ImportScriptsRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		s.writeError(w, http.StatusBadRequest, "invalid JSON: "+err.Error())
		return
	}
	response, err := s.api.ImportScripts(req)
	if err != nil {
		s.writeError(w, http.StatusBadRequest, err.Error())
		return
	}
	s.writeJSON(w, http.StatusOK, response)
}

func (s *Server) handleDeleteScript(w http.ResponseWriter, r *http.Request) {
	scriptID := r.PathValue("id")
	if scriptID == "" {
		s.writeError(w, http.StatusBadRequest, "missing script id")
		return
	}
	entry, err := s.api.DeleteScript(scriptID)
	if err != nil {
		s.writeError(w, http.StatusBadRequest, err.Error())
		return
	}
	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":           true,
		"entry":        entry,
		"generated_at": time.Now().UTC().Format(time.RFC3339),
	})
}

func (s *Server) handleListScriptTrash(w http.ResponseWriter, r *http.Request) {
	entries, err := s.api.ListScriptTrash()
	if err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}
	s.writeJSON(w, http.StatusOK, model.ScriptTrashResponse{
		OK:          true,
		Count:       len(entries),
		Entries:     entries,
		GeneratedAt: time.Now().UTC().Format(time.RFC3339),
	})
}

func (s *Server) handleRestoreScriptTrash(w http.ResponseWriter, r *http.Request) {
	entryID := r.PathValue("id")
	if entryID == "" {
		s.writeError(w, http.StatusBadRequest, "missing trash entry id")
		return
	}
	entry, err := s.api.RestoreScriptTrash(entryID)
	if err != nil {
		s.writeError(w, http.StatusBadRequest, err.Error())
		return
	}
	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":           true,
		"entry":        entry,
		"generated_at": time.Now().UTC().Format(time.RFC3339),
	})
}

func (s *Server) handlePurgeScriptTrash(w http.ResponseWriter, r *http.Request) {
	entryID := r.PathValue("id")
	if entryID == "" {
		s.writeError(w, http.StatusBadRequest, "missing trash entry id")
		return
	}
	if err := s.api.PurgeScriptTrash(entryID); err != nil {
		s.writeError(w, http.StatusBadRequest, err.Error())
		return
	}
	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":           true,
		"entry_id":     entryID,
		"generated_at": time.Now().UTC().Format(time.RFC3339),
	})
}

func (s *Server) handleEmptyScriptTrash(w http.ResponseWriter, r *http.Request) {
	removed, err := s.api.EmptyScriptTrash()
	if err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}
	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":           true,
		"removed":      removed,
		"generated_at": time.Now().UTC().Format(time.RFC3339),
	})
}
