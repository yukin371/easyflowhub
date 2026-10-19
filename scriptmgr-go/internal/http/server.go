package http

import (
	"encoding/json"
	"net/http"
	"strconv"
	"time"

	"scriptmgr/internal/extensions"
	"scriptmgr/internal/localapi"
	"scriptmgr/internal/mcp"
	"scriptmgr/internal/mcpcli"
	"scriptmgr/internal/model"
	"scriptmgr/internal/relay"
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
	// Script file management (roots, source editing, import, trash)
	ListRootDetails() ([]string, []string, error)
	AddRoot(path string) (string, error)
	RemoveRoot(path string) error
	ReadScriptSource(scriptID string) (model.ScriptSource, error)
	WriteScriptSource(scriptID, content string) (model.ScriptSource, error)
	CreateScript(req model.CreateScriptRequest) (model.ScriptRecord, error)
	ImportScripts(req model.ImportScriptsRequest) (model.ImportScriptsResponse, error)
	DeleteScript(scriptID string) (model.ScriptTrashEntry, error)
	ListScriptTrash() ([]model.ScriptTrashEntry, error)
	RestoreScriptTrash(entryID string) (model.ScriptTrashEntry, error)
	PurgeScriptTrash(entryID string) error
	EmptyScriptTrash() (int, error)
}

// ExtensionCatalog is the narrow read surface the HTTP layer needs to serve
// the extension list, extension contributions and the MCP server catalog (F13).
// It is deliberately separate from relay.Service so those endpoints do not
// detour through relay: relay only consumes its own provider/route
// contributions, while manifest discovery and the MCP catalog are served
// independently of whether relay is initialised.
type ExtensionCatalog interface {
	ListExtensionsForAPI() ([]extensions.ListedExtension, []string, error)
	EffectiveContributionsForAPI() (extensions.EffectiveContributions, error)
	EffectiveContributions() (extensions.EffectiveContributions, error)
	ExtensionRoots() []string
}

// Server handles HTTP API requests
type Server struct {
	api     API
	mux     *http.ServeMux
	handler http.Handler
	hub     *Hub
	// router is the single owner of MCP tool-loading state (F09). The HTTP
	// handlers delegate to it instead of tracking a parallel boolean map, so
	// the reported "loaded" flag always reflects the tools that are actually
	// registered for tools/call.
	router *mcp.ToolRouter
	// catalog serves extension and MCP-catalog queries independently of relay.
	catalog ExtensionCatalog
	relay   *relay.Service
}

// NewServer creates a new HTTP server
func NewServer(api API) *Server {
	return NewServerWithRelay(api, nil)
}

// NewServerWithRelay creates a new HTTP server with optional relay integration.
// relay.Service satisfies ExtensionCatalog, so it doubles as the catalog by
// default; use NewServerWithCatalog when extension queries must work without a
// relay service (F13).
func NewServerWithRelay(api API, relayService *relay.Service) *Server {
	var catalog ExtensionCatalog
	if relayService != nil {
		catalog = relayService
	}
	return newServer(api, relayService, catalog)
}

// NewServerWithCatalog creates a server whose extension / MCP-catalog queries
// are served by an injected catalog, independently of relay. This keeps
// /api/extensions and /api/mcp/servers reachable when relay is not initialised.
func NewServerWithCatalog(api API, catalog ExtensionCatalog) *Server {
	return newServer(api, nil, catalog)
}

func newServer(api API, relayService *relay.Service, catalog ExtensionCatalog) *Server {
	s := &Server{
		api:     api,
		mux:     http.NewServeMux(),
		hub:     NewHub(),
		relay:   relayService,
		catalog: catalog,
		router:  mcp.NewToolRouter(api, mcp.DefaultRouterConfig()),
	}
	s.routes()
	s.handler = localapi.WithBrowserOrigins(s.mux)
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
	// Script file management
	s.mux.HandleFunc("GET /api/roots", s.handleListRoots)
	s.mux.HandleFunc("POST /api/roots", s.handleAddRoot)
	s.mux.HandleFunc("DELETE /api/roots", s.handleRemoveRoot)
	s.mux.HandleFunc("GET /api/scripts/{id}/source", s.handleReadScriptSource)
	s.mux.HandleFunc("PUT /api/scripts/{id}/source", s.handleWriteScriptSource)
	s.mux.HandleFunc("POST /api/scripts", s.handleCreateScript)
	s.mux.HandleFunc("POST /api/scripts/import", s.handleImportScripts)
	s.mux.HandleFunc("DELETE /api/scripts/{id}", s.handleDeleteScript)
	s.mux.HandleFunc("GET /api/trash/scripts", s.handleListScriptTrash)
	s.mux.HandleFunc("DELETE /api/trash/scripts", s.handleEmptyScriptTrash)
	s.mux.HandleFunc("POST /api/trash/scripts/{id}/restore", s.handleRestoreScriptTrash)
	s.mux.HandleFunc("DELETE /api/trash/scripts/{id}", s.handlePurgeScriptTrash)
	// MCP category management endpoints
	s.mux.HandleFunc("GET /api/mcp/categories", s.handleListMCPCategories)
	s.mux.HandleFunc("GET /api/mcp/servers", s.handleListMCPServers)
	s.mux.HandleFunc("POST /api/mcp/load/{category}", s.handleMCPLoadCategory)
	s.mux.HandleFunc("POST /api/mcp/unload/{category}", s.handleMCPUnloadCategory)
	if s.relay != nil {
		s.mux.HandleFunc("GET /api/relay/config", s.handleRelayGetConfig)
		s.mux.HandleFunc("PUT /api/relay/config", s.handleRelayPutConfig)
	}
	// Extension queries are served by the injected catalog, not relay, so they
	// stay reachable when relay is not initialised (F13).
	if s.catalog != nil {
		s.mux.HandleFunc("GET /api/extensions", s.handleListExtensions)
		s.mux.HandleFunc("GET /api/extensions/contributions", s.handleListExtensionContributions)
	}
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
	s.handler.ServeHTTP(w, r)
}

func (s *Server) handleListScripts(w http.ResponseWriter, r *http.Request) {
	search := r.URL.Query().Get("search")

	scripts, roots, err := s.api.ListScripts(search)
	if err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":           true,
		"count":        len(scripts),
		"search":       search,
		"roots":        roots,
		"scripts":      scripts,
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
		"ok":           true,
		"script":       script,
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
		"ok":           true,
		"script":       script,
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
		"ok":           true,
		"count":        len(tasks),
		"status":       status,
		"tasks":        tasks,
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
		"ok":           true,
		"session":      session,
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

// handleListMCPCategories returns all categories with their loaded status.
// The router is the single owner of loading state, so "loaded" here always
// matches what tools/call will actually serve (F09).
func (s *Server) handleListMCPCategories(w http.ResponseWriter, r *http.Request) {
	categories, err := s.router.ListCategories()
	if err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":           true,
		"categories":   categories,
		"generated_at": time.Now().UTC().Format(time.RFC3339),
	})
}

func (s *Server) handleListMCPServers(w http.ResponseWriter, r *http.Request) {
	effectiveContributions := extensions.EffectiveContributions{}
	if s.catalog != nil {
		var err error
		effectiveContributions, err = s.catalog.EffectiveContributions()
		if err != nil {
			s.writeError(w, http.StatusInternalServerError, err.Error())
			return
		}
	}

	catalog, err := mcpcli.LoadEffectiveCatalog(effectiveContributions)
	if err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":      true,
		"catalog": catalog,
	})
}

// handleMCPLoadCategory loads a category's script tools through the router.
// This registers the real handlers, so a reported "loaded" state corresponds
// to tools that tools/call will actually serve (F09).
func (s *Server) handleMCPLoadCategory(w http.ResponseWriter, r *http.Request) {
	category := r.PathValue("category")
	if category == "" {
		s.writeError(w, http.StatusBadRequest, "missing category name")
		return
	}

	result, err := s.router.LoadCategory(category)
	if err != nil {
		s.writeError(w, http.StatusNotFound, err.Error())
		return
	}

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":              true,
		"category":        result.Category,
		"loaded":          true,
		"already_loaded":  result.AlreadyLoaded,
		"script_count":    result.ScriptCount,
		"total_tools":     result.TotalTools,
		"evicted_categories": result.EvictedCategories,
	})
}

// handleMCPUnloadCategory unloads a category's script tools through the
// router, deregistering the handlers so the tools stop being served.
func (s *Server) handleMCPUnloadCategory(w http.ResponseWriter, r *http.Request) {
	category := r.PathValue("category")
	if category == "" {
		s.writeError(w, http.StatusBadRequest, "missing category name")
		return
	}

	if err := s.router.UnloadCategory(category); err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":       true,
		"category": category,
		"loaded":   false,
	})
}

func (s *Server) handleRelayGetConfig(w http.ResponseWriter, r *http.Request) {
	snapshot, err := s.relay.Snapshot()
	if err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":       true,
		"snapshot": snapshot,
	})
}

func (s *Server) handleRelayPutConfig(w http.ResponseWriter, r *http.Request) {
	var cfg relay.Config
	if err := json.NewDecoder(r.Body).Decode(&cfg); err != nil {
		s.writeError(w, http.StatusBadRequest, "invalid JSON: "+err.Error())
		return
	}

	if err := s.relay.SaveConfig(cfg); err != nil {
		s.writeError(w, http.StatusBadRequest, err.Error())
		return
	}

	s.handleRelayGetConfig(w, r)
}

func (s *Server) handleListExtensions(w http.ResponseWriter, r *http.Request) {
	items, roots, err := s.catalog.ListExtensionsForAPI()
	if err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":         true,
		"roots":      roots,
		"count":      len(items),
		"extensions": items,
	})
}

func (s *Server) handleListExtensionContributions(w http.ResponseWriter, r *http.Request) {
	contributions, err := s.catalog.EffectiveContributionsForAPI()
	if err != nil {
		s.writeError(w, http.StatusInternalServerError, err.Error())
		return
	}

	s.writeJSON(w, http.StatusOK, map[string]any{
		"ok":            true,
		"roots":         s.catalog.ExtensionRoots(),
		"contributions": contributions,
	})
}
