package http

import (
	"encoding/json"
	"net/http"
	"strconv"
	"sync"
	"time"

	"scriptmgr/internal/model"
)

// API defines the interface for script operations needed by HTTP server
type API interface {
	ListScripts(search string) ([]model.ScriptRecord, []string, error)
	DescribeScript(scriptID string) (model.ScriptRecord, error)
	RunScript(scriptID string, args []string, opts model.RunOptions) (any, error)
	UpdateScriptCategory(scriptID, category string) error
	GetTask(taskID string) (*model.Task, error)
	ListTasks(status string, limit int) ([]model.Task, error)
	ReadTaskLog(taskID string, offset, limit int, tail bool) (string, *model.Task, error)
	CancelSession(sessionID string) (model.SessionRecord, error)
	CancelTask(taskID string) (model.SessionRecord, error)
	ListCategories() (map[string]int, error)
}

// Server handles HTTP API requests
type Server struct {
	api  API
	mux  *http.ServeMux
	hub  *Hub
	mcp  *MCPServer
}

// MCPServer handles MCP-related state tracking
type MCPServer struct {
	mu           sync.RWMutex
	loadedCategories map[string]bool
	api           API
}

// NewServer creates a new HTTP server
func NewServer(api API) *Server {
	s := &Server{
		api:  api,
		mux:  http.NewServeMux(),
		hub:  NewHub(),
		mcp:  &MCPServer{
			loadedCategories: make(map[string]bool),
			api:              api,
		},
	}
	s.routes()
	return s
}

func (s *Server) routes() {
	s.mux.HandleFunc("GET /api/scripts", s.handleListScripts)
	s.mux.HandleFunc("GET /api/scripts/{id}", s.handleDescribeScript)
	s.mux.HandleFunc("PATCH /api/scripts/{id}", s.handleUpdateScript)
	s.mux.HandleFunc("POST /api/run", s.handleRunScript)
	s.mux.HandleFunc("GET /api/tasks", s.handleListTasks)
	s.mux.HandleFunc("GET /api/tasks/{id}", s.handleGetTask)
	s.mux.HandleFunc("GET /api/tasks/{id}/log", s.handleTaskLog)
	s.mux.HandleFunc("POST /api/cancel/{id}", s.handleCancel)
	s.mux.HandleFunc("GET /ws", ServeWS(s.hub, DefaultUpgrader))
	// MCP category management endpoints
	s.mux.HandleFunc("GET /api/mcp/categories", s.handleListMCPCategories)
	s.mux.HandleFunc("POST /api/mcp/load/{category}", s.handleMCPLoadCategory)
	s.mux.HandleFunc("POST /api/mcp/unload/{category}", s.handleMCPUnloadCategory)
}

// Hub returns the WebSocket hub for external integration
func (s *Server) Hub() *Hub {
	return s.hub
}

// StartHub starts the WebSocket hub's run loop in a goroutine
func (s *Server) StartHub() {
	go s.hub.Run()
}

// ServeHTTP implements http.Handler interface
func (s *Server) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	// Add CORS headers for Tauri
	w.Header().Set("Access-Control-Allow-Origin", "*")
	w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PATCH, OPTIONS")
	w.Header().Set("Access-Control-Allow-Headers", "Content-Type")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	s.mux.ServeHTTP(w, r)
}

func (s *Server) handleListScripts(w http.ResponseWriter, r *http.Request) {
	search := r.URL.Query().Get("search")

	scripts, roots, err := s.api.ListScripts(search)
	if err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":          true,
		"count":       len(scripts),
		"search":      search,
		"roots":       roots,
		"scripts":     scripts,
		"generated_at": time.Now().UTC().Format(time.RFC3339),
	})
}

func (s *Server) handleDescribeScript(w http.ResponseWriter, r *http.Request) {
	scriptID := r.PathValue("id")
	if scriptID == "" {
		s.writeError(w, http.StatusBadRequest, "missing script id")
		return
	}

	script, err := s.api.DescribeScript(scriptID)
	if err != nil {
		s.writeError(w, http.StatusNotFound, err.Error())
		return
	}

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":          true,
		"script":      script,
		"generated_at": time.Now().UTC().Format(time.RFC3339),
	})
}

func (s *Server) handleUpdateScript(w http.ResponseWriter, r *http.Request) {
	scriptID := r.PathValue("id")
	if scriptID == "" {
		s.writeError(w, http.StatusBadRequest, "missing script id")
		return
	}

	var req struct {
		Category string `json:"category"`
	}

	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		s.writeError(w, http.StatusBadRequest, "invalid JSON: "+err.Error())
		return
	}

	if err := s.api.UpdateScriptCategory(scriptID, req.Category); err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}

	// Return updated script
	script, err := s.api.DescribeScript(scriptID)
	if err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":          true,
		"script":      script,
		"generated_at": time.Now().UTC().Format(time.RFC3339),
	})
}

func (s *Server) handleRunScript(w http.ResponseWriter, r *http.Request) {
	var req struct {
		ScriptID string   `json:"script_id"`
		Args     []string `json:"args"`
		Async    bool     `json:"async"`
		DryRun   bool     `json:"dry_run"`
	}

	if r.Body != nil && r.ContentLength > 0 {
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			s.writeError(w, http.StatusBadRequest, "invalid JSON: "+err.Error())
			return
		}
	}

	if req.ScriptID == "" {
		s.writeError(w, http.StatusBadRequest, "missing script_id")
		return
	}

	opts := model.RunOptions{
		Detach:        req.Async,
		DryRun:        req.DryRun,
		CaptureOutput: !req.Async,
	}

	result, err := s.api.RunScript(req.ScriptID, req.Args, opts)
	if err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}

	s.writeJSON(w, http.StatusOK, result)
}

func (s *Server) handleListTasks(w http.ResponseWriter, r *http.Request) {
	status := r.URL.Query().Get("status")
	limit := 50
	if l := r.URL.Query().Get("limit"); l != "" {
		if parsed, err := strconv.Atoi(l); err == nil && parsed > 0 {
			limit = parsed
		}
	}

	tasks, err := s.api.ListTasks(status, limit)
	if err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":          true,
		"count":       len(tasks),
		"status":      status,
		"tasks":       tasks,
		"generated_at": time.Now().UTC().Format(time.RFC3339),
	})
}

func (s *Server) handleGetTask(w http.ResponseWriter, r *http.Request) {
	taskID := r.PathValue("id")
	if taskID == "" {
		s.writeError(w, http.StatusBadRequest, "missing task id")
		return
	}

	task, err := s.api.GetTask(taskID)
	if err != nil {
		s.writeError(w, http.StatusNotFound, err.Error())
		return
	}

	s.writeJSON(w, http.StatusOK, task)
}

func (s *Server) handleTaskLog(w http.ResponseWriter, r *http.Request) {
	taskID := r.PathValue("id")
	if taskID == "" {
		s.writeError(w, http.StatusBadRequest, "missing task id")
		return
	}

	offset := 0
	limit := 100
	tail := r.URL.Query().Get("tail") == "true"

	if o := r.URL.Query().Get("offset"); o != "" {
		if parsed, err := strconv.Atoi(o); err == nil && parsed >= 0 {
			offset = parsed
		}
	}
	if l := r.URL.Query().Get("limit"); l != "" {
		if parsed, err := strconv.Atoi(l); err == nil && parsed > 0 {
			limit = parsed
		}
	}

	content, task, err := s.api.ReadTaskLog(taskID, offset, limit, tail)
	if err != nil {
		s.writeError(w, http.StatusNotFound, err.Error())
		return
	}

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":      true,
		"task_id": taskID,
		"task":    task,
		"content": content,
		"offset":  offset,
		"limit":   limit,
		"tail":    tail,
	})
}

func (s *Server) handleCancel(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if id == "" {
		s.writeError(w, http.StatusBadRequest, "missing id")
		return
	}

	// Try canceling as task first, then as session
	session, err := s.api.CancelTask(id)
	if err != nil {
		session, err = s.api.CancelSession(id)
		if err != nil {
			s.writeError(w, http.StatusNotFound, err.Error())
			return
		}
	}

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":          true,
		"session":     session,
		"generated_at": time.Now().UTC().Format(time.RFC3339),
	})
}

func (s *Server) writeJSON(w http.ResponseWriter, status int, data any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(data)
}

func (s *Server) writeError(w http.ResponseWriter, status int, message string) {
	s.writeJSON(w, status, map[string]any{
		"ok":    false,
		"error": message,
	})
}

// handleListMCPCategories returns all categories with their loaded status
func (s *Server) handleListMCPCategories(w http.ResponseWriter, r *http.Request) {
	// Get all categories from scripts
	categories, err := s.api.ListCategories()
	if err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}

	s.mcp.mu.RLock()
	loaded := s.mcp.loadedCategories
	s.mcp.mu.RUnlock()

	// Build category info list
	var result []map[string]any
	for name, count := range categories {
		result = append(result, map[string]any{
			"name":   name,
			"count":  count,
			"loaded": loaded[name],
		})
	}

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":         true,
		"categories":  result,
		"generated_at": time.Now().UTC().Format(time.RFC3339),
	})
}

// handleMCPLoadCategory marks a category as loaded
func (s *Server) handleMCPLoadCategory(w http.ResponseWriter, r *http.Request) {
	category := r.PathValue("category")
	if category == "" {
		s.writeError(w, http.StatusBadRequest, "missing category name")
		return
	}

	s.mcp.mu.Lock()
	s.mcp.loadedCategories[category] = true
	s.mcp.mu.Unlock()

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":       true,
		"category":  category,
		"loaded":   true,
	})
}

// handleMCPUnloadCategory marks a category as unloaded
func (s *Server) handleMCPUnloadCategory(w http.ResponseWriter, r *http.Request) {
	category := r.PathValue("category")
	if category == "" {
		s.writeError(w, http.StatusBadRequest, "missing category name")
		return
	}

	s.mcp.mu.Lock()
	delete(s.mcp.loadedCategories, category)
	s.mcp.mu.Unlock()

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":       true,
		"category":  category,
		"loaded":   false,
	})
}
