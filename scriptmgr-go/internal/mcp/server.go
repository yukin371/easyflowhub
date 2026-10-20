package mcp

import (
	"log/slog"

	"scriptmgr/internal/model"
	"scriptmgr/internal/notes"
)

// ScriptAPI defines the interface for script operations needed by MCP
type ScriptAPI interface {
	ListScripts(search string) ([]model.ScriptRecord, []string, error)
	RunScript(scriptID string, args []string, opts model.RunOptions) (any, error)
	GetTask(taskID string) (*model.Task, error)
	ListTasks(status string, limit int) ([]model.Task, error)
	ReadTaskLog(taskID string, offset, limit int, tail bool) (string, *model.Task, error)
}

// NotesAPI defines the interface for notes operations needed by MCP
type NotesAPI interface {
	GetRepoPath() string
	SetRepoPath(path string) error
	Sync(direction notes.SyncDirection) (*notes.SyncReport, error)
	ListNotes(search string) ([]*notes.Note, error)
	GetNote(noteID string) (*notes.Note, error)
	RepoExists() bool
}

// Notifier defines the interface for sending notifications
type Notifier interface {
	WriteMessage(msg []byte) error
}

// Request represents a JSON-RPC 2.0 request
type Request struct {
	JSONRPC string         `json:"jsonrpc"`
	ID      *int           `json:"id,omitempty"`
	Method  string         `json:"method"`
	Params  map[string]any `json:"params,omitempty"`
}

// Response represents a JSON-RPC 2.0 response
type Response struct {
	JSONRPC string         `json:"jsonrpc"`
	ID      *int           `json:"id,omitempty"`
	Result  any            `json:"result,omitempty"`
	Error   *ResponseError `json:"error,omitempty"`
}

// ResponseError represents a JSON-RPC error
type ResponseError struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
}

// Tool represents an MCP tool definition
type Tool struct {
	Name        string `json:"name"`
	Description string `json:"description"`
}

// Server handles MCP protocol requests
type Server struct {
	api       ScriptAPI
	notesAPI  NotesAPI
	transport Notifier
	router    *ToolRouter
}

// New creates a new MCP server
func New() *Server {
	return &Server{}
}

// WithAPI returns a server with API access
func (s *Server) WithAPI(api ScriptAPI) *Server {
	s.api = api
	// Initialize router with API
	s.router = NewToolRouter(api, DefaultRouterConfig())
	// Register static tools
	s.registerStaticTools()
	return s
}

// registerStaticTools wires every implemented static tool into the router,
// pairing the single schema source (GetToolSchemas) with its handler. The
// router is the one runtime registry for schema + handler (F11); a schema
// without a wired handler is skipped rather than advertised as a tool that
// cannot run.
func (s *Server) registerStaticTools() {
	handlers := map[string]ToolHandler{
		"list_scripts":    s.callListScripts,
		"run_script":      s.callRunScript,
		"list_categories": s.callListCategories,
		"load_category":   s.callLoadCategory,
		"unload_category": s.callUnloadCategory,
		"search_scripts":  s.callSearchScripts,
		"get_notes_repo":  s.callGetNotesRepo,
		"set_notes_repo":  s.callSetNotesRepo,
		"sync_notes":      s.callSyncNotes,
		"list_notes":      s.callListNotes,
		"get_note":        s.callGetNote,
		"get_task_result": s.callGetTaskResult,
		"read_log":        s.callReadLog,
	}
	for _, schema := range GetToolSchemas() {
		handler, ok := handlers[schema.Name]
		if !ok {
			slog.Warn("mcp: schema has no wired handler; skipping registration", "tool", schema.Name)
			continue
		}
		s.router.RegisterStatic(schema.Name, schema, handler)
	}
}

// WithNotesAPI returns a server with notes API access
func (s *Server) WithNotesAPI(notesAPI NotesAPI) *Server {
	s.notesAPI = notesAPI
	return s
}

// SetTransport sets the transport for sending notifications
func (s *Server) SetTransport(n Notifier) {
	s.transport = n
}
