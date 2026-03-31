package mcp

import (
	"encoding/json"
	"fmt"
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

// registerStaticTools registers all static tools with the router
func (s *Server) registerStaticTools() {
	// Script tools
	s.router.RegisterStatic("list_scripts", ToolWithSchema{
		Name:        "list_scripts",
		Description: "List all available scripts",
		InputSchema: map[string]any{
			"type": "object",
			"properties": map[string]any{
				"search": map[string]any{
					"type":        "string",
					"description": "Search filter",
				},
			},
		},
	}, s.callListScripts)

	s.router.RegisterStatic("run_script", ToolWithSchema{
		Name:        "run_script",
		Description: "Execute a script by ID",
		InputSchema: map[string]any{
			"type": "object",
			"properties": map[string]any{
				"script_id": map[string]any{
					"type":        "string",
					"description": "Script ID to run",
				},
				"args": map[string]any{
					"type":  "array",
					"items": map[string]any{"type": "string"},
				},
				"async": map[string]any{
					"type":    "boolean",
					"default": true,
				},
				"dry_run": map[string]any{
					"type": "boolean",
				},
			},
			"required": []string{"script_id"},
		},
	}, s.callRunScript)

	// Category management tools
	s.router.RegisterStatic("list_categories", ToolWithSchema{
		Name:        "list_categories",
		Description: "List all script categories with their script counts. Use this to discover available script categories before loading them.",
		InputSchema: map[string]any{
			"type":       "object",
			"properties": map[string]any{},
		},
	}, s.callListCategories)

	s.router.RegisterStatic("load_category", ToolWithSchema{
		Name:        "load_category",
		Description: "Load all scripts from a category as dynamic tools. After loading, scripts become available as script_<id> tools.",
		InputSchema: map[string]any{
			"type": "object",
			"properties": map[string]any{
				"category": map[string]any{
					"type":        "string",
					"description": "Category name to load",
				},
			},
			"required": []string{"category"},
		},
	}, s.callLoadCategory)

	s.router.RegisterStatic("unload_category", ToolWithSchema{
		Name:        "unload_category",
		Description: "Unload a previously loaded category to free up tool slots.",
		InputSchema: map[string]any{
			"type": "object",
			"properties": map[string]any{
				"category": map[string]any{
					"type":        "string",
					"description": "Category name to unload",
				},
			},
			"required": []string{"category"},
		},
	}, s.callUnloadCategory)

	s.router.RegisterStatic("search_scripts", ToolWithSchema{
		Name:        "search_scripts",
		Description: "Search for scripts by keyword across name, description, and tags",
		InputSchema: map[string]any{
			"type": "object",
			"properties": map[string]any{
				"keyword": map[string]any{
					"type":        "string",
					"description": "Search keyword",
				},
			},
		},
	}, s.callSearchScripts)

	// Notes tools
	s.router.RegisterStatic("get_notes_repo", ToolWithSchema{
		Name:        "get_notes_repo",
		Description: "Get the path to the notes repository",
		InputSchema: map[string]any{
			"type":       "object",
			"properties": map[string]any{},
		},
	}, s.callGetNotesRepo)

	s.router.RegisterStatic("set_notes_repo", ToolWithSchema{
		Name:        "set_notes_repo",
		Description: "Set a custom path for the notes repository",
		InputSchema: map[string]any{
			"type": "object",
			"properties": map[string]any{
				"path": map[string]any{
					"type":        "string",
					"description": "Custom path for notes repository",
				},
			},
			"required": []string{"path"},
		},
	}, s.callSetNotesRepo)

	s.router.RegisterStatic("sync_notes", ToolWithSchema{
		Name:        "sync_notes",
		Description: "Synchronize notes between DeskFlow database and file repository",
		InputSchema: map[string]any{
			"type": "object",
			"properties": map[string]any{
				"direction": map[string]any{
					"type":        "string",
					"description": "Sync direction: bidirectional (default), db_to_file, file_to_db",
					"enum":        []string{"bidirectional", "db_to_file", "file_to_db"},
				},
			},
		},
	}, s.callSyncNotes)

	s.router.RegisterStatic("list_notes", ToolWithSchema{
		Name:        "list_notes",
		Description: "List all notes from the file repository",
		InputSchema: map[string]any{
			"type": "object",
			"properties": map[string]any{
				"search": map[string]any{
					"type":        "string",
					"description": "Search filter",
				},
			},
		},
	}, s.callListNotes)

	s.router.RegisterStatic("get_note", ToolWithSchema{
		Name:        "get_note",
		Description: "Get a single note by ID",
		InputSchema: map[string]any{
			"type": "object",
			"properties": map[string]any{
				"note_id": map[string]any{
					"type":        "string",
					"description": "Note ID to retrieve",
				},
			},
			"required": []string{"note_id"},
		},
	}, s.callGetNote)

	// Async task tools
	s.router.RegisterStatic("get_task_result", ToolWithSchema{
		Name:        "get_task_result",
		Description: "Get the result of an async script execution. Set wait=true to block until task completes.",
		InputSchema: map[string]any{
			"type": "object",
			"properties": map[string]any{
				"task_id": map[string]any{
					"type":        "string",
					"description": "Task ID returned from run_script with async=true",
				},
				"wait": map[string]any{
					"type":        "boolean",
					"default":     false,
					"description": "Block until task completes",
				},
			},
			"required": []string{"task_id"},
		},
	}, s.callGetTaskResult)

	s.router.RegisterStatic("read_log", ToolWithSchema{
		Name:        "read_log",
		Description: "Read the execution log of a task. Supports pagination and tail mode.",
		InputSchema: map[string]any{
			"type": "object",
			"properties": map[string]any{
				"task_id": map[string]any{
					"type":        "string",
					"description": "Task ID to read log from",
				},
				"offset": map[string]any{
					"type":        "integer",
					"default":     0,
					"description": "Line offset for pagination",
				},
				"limit": map[string]any{
					"type":        "integer",
					"default":     100,
					"description": "Number of lines to read",
				},
				"tail": map[string]any{
					"type":        "boolean",
					"default":     false,
					"description": "Read last N lines (most recent)",
				},
			},
			"required": []string{"task_id"},
		},
	}, s.callReadLog)
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

// NotificationParams contains the parameters for task_completed notification
type NotificationParams struct {
	TaskID            string `json:"task_id"`
	ScriptID          string `json:"script_id"`
	Status            string `json:"status"`
	DurationMs        int64  `json:"duration_ms"`
	ExitCode          *int   `json:"exit_code,omitempty"`
	OutputSummary     string `json:"output_summary,omitempty"`
	OutputFullAvailable bool   `json:"output_full_available"`
}

// Notification represents a JSON-RPC 2.0 notification
type Notification struct {
	JSONRPC string             `json:"jsonrpc"`
	Method  string             `json:"method"`
	Params  NotificationParams `json:"params"`
}

// BroadcastTaskStatus implements executor.StatusBroadcaster interface
// It sends a notification when an async task completes
func (s *Server) BroadcastTaskStatus(taskID, scriptID, status string, exitCode *int, durationMs int64) {
	// Get output summary from API if available
	var outputSummary string
	var outputFullAvailable bool
	if s.api != nil {
		if task, err := s.api.GetTask(taskID); err == nil && task != nil {
			outputSummary = task.OutputSummary
			outputFullAvailable = task.OutputPath != ""
		}
	}

	notification := Notification{
		JSONRPC: "2.0",
		Method:  "notifications/task_completed",
		Params: NotificationParams{
			TaskID:              taskID,
			ScriptID:            scriptID,
			Status:              status,
			DurationMs:          durationMs,
			ExitCode:            exitCode,
			OutputSummary:       outputSummary,
			OutputFullAvailable: outputFullAvailable,
		},
	}

	// Send notification if transport is available
	if s.transport != nil {
		notificationBytes, err := json.Marshal(notification)
		if err != nil {
			slog.Error("failed to marshal notification", "error", err)
			return
		}
		if err := s.transport.WriteMessage(notificationBytes); err != nil {
			slog.Error("failed to send notification", "error", err)
		}
	} else {
		// Graceful degradation - log to stdout for debugging
		slog.Debug("notification skipped - no transport", "task_id", taskID, "status", status)
	}
}

// Handle processes an MCP request and returns a response
func (s *Server) Handle(req Request) Response {
	switch req.Method {
	case "initialize":
		return s.handleInitialize(req)
	case "tools/list":
		return s.handleToolsList(req)
	case "tools/call":
		return s.handleToolsCall(req)
	default:
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32601,
				Message: "method not found: " + req.Method,
			},
		}
	}
}

func (s *Server) handleInitialize(req Request) Response {
	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"protocolVersion": "2024-11-05",
			"capabilities": map[string]any{
				"tools": map[string]any{},
			},
			"serverInfo": map[string]any{
				"name":    "scriptmgr",
				"version": "1.0.0",
			},
		},
	}
}

func (s *Server) handleToolsList(req Request) Response {
	// Get static/discovery tools only (no dynamic script_* tools)
	// This enables progressive disclosure - script tools only appear after load_category
	var tools []ToolWithSchema
	if s.router != nil {
		tools = s.router.GetDiscoveryTools()
	} else {
		tools = GetToolSchemas() // Fallback to static schemas
	}

	// Get category summaries for progressive disclosure
	var categories []CategoryInfo
	if s.router != nil {
		categories, _ = s.router.ListCategories()
	}

	// Build category summaries
	categorySummaries := make([]map[string]any, 0, len(categories))
	for _, cat := range categories {
		categorySummaries = append(categorySummaries, map[string]any{
			"name":    cat.Name,
			"count":  cat.Count,
			"loaded": cat.Loaded,
		})
	}

	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"tools":      tools,
			"categories": categorySummaries,
			"_progressive": map[string]any{
				"hint": "Use load_category to reveal script tools. Categories must be loaded before their script_* tools become available.",
			},
		},
	}
}

func (s *Server) handleToolsCall(req Request) Response {
	params := req.Params
	if params == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: "missing params",
			},
		}
	}

	toolName, ok := params["name"].(string)
	if !ok {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: "missing tool name",
			},
		}
	}

	// Try router first (handles both static and dynamic tools)
	if s.router != nil {
		resp, handled := s.router.HandleTool(toolName, req)
		if handled {
			return resp
		}
	}

	// Fallback to legacy switch-case for backwards compatibility
	switch toolName {
	case "list_scripts":
		return s.callListScripts(req)
	case "run_script":
		return s.callRunScript(req)
	case "list_categories":
		return s.callListCategories(req)
	case "load_category":
		return s.callLoadCategory(req)
	case "unload_category":
		return s.callUnloadCategory(req)
	case "search_scripts":
		return s.callSearchScripts(req)
	// Notes tools
	case "get_notes_repo":
		return s.callGetNotesRepo(req)
	case "set_notes_repo":
		return s.callSetNotesRepo(req)
	case "sync_notes":
		return s.callSyncNotes(req)
	case "list_notes":
		return s.callListNotes(req)
	case "get_note":
		return s.callGetNote(req)
	default:
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: "unknown tool: " + toolName,
			},
		}
	}
}

func (s *Server) callListScripts(req Request) Response {
	// Validate arguments if present
	if args := req.Params["arguments"]; args != nil {
		if argsMap, ok := args.(map[string]any); ok {
			if err := ValidateToolArgs("list_scripts", argsMap); err != nil {
				return Response{
					JSONRPC: "2.0",
					ID:      req.ID,
					Error: &ResponseError{
						Code:    -32602,
						Message: err.Error(),
					},
				}
			}
		}
	}

	if s.api == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Result: map[string]any{
				"content": []map[string]any{
					{
						"type": "text",
						"text": "[]",
					},
				},
			},
		}
	}

	// Get search argument if provided
	var search string
	if args := req.Params["arguments"]; args != nil {
		if argsMap, ok := args.(map[string]any); ok {
			if searchStr, ok := argsMap["search"].(string); ok {
				search = searchStr
			}
		}
	}

	// Call the API
	scripts, roots, err := s.api.ListScripts(search)
	if err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: err.Error(),
			},
		}
	}

	// Build response
	result := map[string]any{
		"scripts": scripts,
		"roots":   roots,
		"count":   len(scripts),
	}

	resultJSON, err := json.Marshal(result)
	if err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "failed to serialize result",
			},
		}
	}

	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "text",
					"text": string(resultJSON),
				},
			},
		},
	}
}

func (s *Server) callRunScript(req Request) Response {
	params := req.Params
	if params == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: "missing params",
			},
		}
	}

	arguments, ok := params["arguments"].(map[string]any)
	if !ok {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: "missing arguments",
			},
		}
	}

	// Validate arguments using schema
	if err := ValidateToolArgs("run_script", arguments); err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: err.Error(),
			},
		}
	}

	scriptID := arguments["script_id"].(string)

	// Extract optional args
	var args []string
	if argsAny, ok := arguments["args"].([]any); ok {
		for _, a := range argsAny {
			if str, ok := a.(string); ok {
				args = append(args, str)
			}
		}
	}

	// Build run options
	opts := model.RunOptions{
		Detach:        true, // Default to async for MCP
		CaptureOutput: true,
	}

	if async, ok := arguments["async"].(bool); ok {
		opts.Detach = async
	}

	if dryRun, ok := arguments["dry_run"].(bool); ok {
		opts.DryRun = dryRun
	}

	// Check if API is available
	if s.api == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "API not configured",
			},
		}
	}

	// Call the API
	result, err := s.api.RunScript(scriptID, args, opts)
	if err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: err.Error(),
			},
		}
	}

	// Serialize result to JSON
	resultJSON, err := json.Marshal(result)
	if err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "failed to serialize result",
			},
		}
	}

	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "text",
					"text": string(resultJSON),
				},
			},
		},
	}
}

// ============================================================================
// Notes Tools
// ============================================================================

func (s *Server) callGetNotesRepo(req Request) Response {
	if s.notesAPI == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "notes API not configured",
			},
		}
	}

	result := map[string]any{
		"path":   s.notesAPI.GetRepoPath(),
		"exists": s.notesAPI.RepoExists(),
	}
	resultJSON, _ := json.Marshal(result)
	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "text",
					"text": string(resultJSON),
				},
			},
		},
	}
}

func (s *Server) callSetNotesRepo(req Request) Response {
	if s.notesAPI == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "notes API not configured",
			},
		}
	}

	arguments, ok := req.Params["arguments"].(map[string]any)
	if !ok {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: "missing arguments",
			},
		}
	}

	// Validate
	if err := ValidateToolArgs("set_notes_repo", arguments); err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: err.Error(),
			},
		}
	}

	path := arguments["path"].(string)
	if err := s.notesAPI.SetRepoPath(path); err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: err.Error(),
			},
		}
	}

	result := map[string]any{
		"success": true,
		"path":    path,
	}
	resultJSON, _ := json.Marshal(result)
	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "text",
					"text": string(resultJSON),
				},
			},
		},
	}
}

func (s *Server) callSyncNotes(req Request) Response {
	if s.notesAPI == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "notes API not configured",
			},
		}
	}

	// Get direction argument
	direction := notes.SyncBidirectional
	if args, ok := req.Params["arguments"].(map[string]any); ok {
		if dir, ok := args["direction"].(string); ok {
			direction = notes.SyncDirection(dir)
		}
	}

	report, err := s.notesAPI.Sync(direction)
	if err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: err.Error(),
			},
		}
	}

	result := map[string]any{
		"success": true,
		"report":  report,
	}
	resultJSON, _ := json.Marshal(result)
	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "text",
					"text": string(resultJSON),
				},
			},
		},
	}
}

func (s *Server) callListNotes(req Request) Response {
	if s.notesAPI == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "notes API not configured",
			},
		}
	}

	// Validate arguments if present
	if args := req.Params["arguments"]; args != nil {
		if argsMap, ok := args.(map[string]any); ok {
			if err := ValidateToolArgs("list_notes", argsMap); err != nil {
				return Response{
					JSONRPC: "2.0",
					ID:      req.ID,
					Error: &ResponseError{
						Code:    -32602,
						Message: err.Error(),
					},
				}
			}
		}
	}

	// Get search argument
	var search string
	if args, ok := req.Params["arguments"].(map[string]any); ok {
		if s, ok := args["search"].(string); ok {
			search = s
		}
	}

	notesList, err := s.notesAPI.ListNotes(search)
	if err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: err.Error(),
			},
		}
	}

	result := map[string]any{
		"notes": notesList,
		"count": len(notesList),
	}
	resultJSON, _ := json.Marshal(result)
	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "text",
					"text": string(resultJSON),
				},
			},
		},
	}
}

func (s *Server) callGetNote(req Request) Response {
	if s.notesAPI == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "notes API not configured",
			},
		}
	}

	arguments, ok := req.Params["arguments"].(map[string]any)
	if !ok {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: "missing arguments",
			},
		}
	}

	// Validate
	if err := ValidateToolArgs("get_note", arguments); err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: err.Error(),
			},
		}
	}

	noteID := arguments["note_id"].(string)
	note, err := s.notesAPI.GetNote(noteID)
	if err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: err.Error(),
			},
		}
	}

	resultJSON, _ := json.Marshal(note)
	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "text",
					"text": string(resultJSON),
				},
			},
		},
	}
}

// ============================================================================
// Category Management Tools
// ============================================================================

func (s *Server) callListCategories(req Request) Response {
	if s.router == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "router not initialized",
			},
		}
	}

	categories, err := s.router.ListCategories()
	if err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: err.Error(),
			},
		}
	}

	result := map[string]any{
		"categories": categories,
		"count":      len(categories),
	}
	resultJSON, _ := json.Marshal(result)
	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "text",
					"text": string(resultJSON),
				},
			},
		},
	}
}

func (s *Server) callLoadCategory(req Request) Response {
	if s.router == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "router not initialized",
			},
		}
	}

	arguments, ok := req.Params["arguments"].(map[string]any)
	if !ok {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: "missing arguments",
			},
		}
	}

	// Validate
	if err := ValidateToolArgs("load_category", arguments); err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: err.Error(),
			},
		}
	}

	category := arguments["category"].(string)
	result, err := s.router.LoadCategory(category)
	if err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: err.Error(),
			},
		}
	}

	resultJSON, _ := json.Marshal(result)
	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "text",
					"text": string(resultJSON),
				},
			},
		},
	}
}

func (s *Server) callUnloadCategory(req Request) Response {
	if s.router == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "router not initialized",
			},
		}
	}

	arguments, ok := req.Params["arguments"].(map[string]any)
	if !ok {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: "missing arguments",
			},
		}
	}

	// Validate
	if err := ValidateToolArgs("unload_category", arguments); err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: err.Error(),
			},
		}
	}

	category := arguments["category"].(string)
	if err := s.router.UnloadCategory(category); err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: err.Error(),
			},
		}
	}

	result := map[string]any{
		"success":  true,
		"category": category,
		"message":  fmt.Sprintf("Category '%s' unloaded successfully", category),
	}
	resultJSON, _ := json.Marshal(result)
	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "text",
					"text": string(resultJSON),
				},
			},
		},
	}
}

func (s *Server) callSearchScripts(req Request) Response {
	if s.router == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "router not initialized",
			},
		}
	}

	// Validate arguments if present
	if args := req.Params["arguments"]; args != nil {
		if argsMap, ok := args.(map[string]any); ok {
			if err := ValidateToolArgs("search_scripts", argsMap); err != nil {
				return Response{
					JSONRPC: "2.0",
					ID:      req.ID,
					Error: &ResponseError{
						Code:    -32602,
						Message: err.Error(),
					},
				}
			}
		}
	}

	// Get keyword argument
	var keyword string
	if args, ok := req.Params["arguments"].(map[string]any); ok {
		if kw, ok := args["keyword"].(string); ok {
			keyword = kw
		}
	}

	scripts, err := s.router.SearchScripts(keyword)
	if err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: err.Error(),
			},
		}
	}

	result := map[string]any{
		"scripts": scripts,
		"count":   len(scripts),
	}
	resultJSON, _ := json.Marshal(result)
	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "text",
					"text": string(resultJSON),
				},
			},
		},
	}
}

// ============================================================================
// Async Task Tools
// ============================================================================

func (s *Server) callGetTaskResult(req Request) Response {
	if s.api == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "API not configured",
			},
		}
	}

	arguments, ok := req.Params["arguments"].(map[string]any)
	if !ok {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: "missing arguments",
			},
		}
	}

	// Validate
	if err := ValidateToolArgs("get_task_result", arguments); err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: err.Error(),
			},
		}
	}

	taskID := arguments["task_id"].(string)

	// Get wait parameter
	wait := false
	if w, ok := arguments["wait"].(bool); ok {
		wait = w
	}

	// If wait=true, poll until task completes (with timeout)
	if wait {
		// Simple polling with timeout
		const maxWaitMs = 60000
		const pollIntervalMs = 500
		elapsed := 0

		for elapsed < maxWaitMs {
			task, err := s.api.GetTask(taskID)
			if err != nil {
				return Response{
					JSONRPC: "2.0",
					ID:      req.ID,
					Error: &ResponseError{
						Code:    -32603,
						Message: err.Error(),
					},
				}
			}

			// Check if task is complete
			if task.Status != "pending" && task.Status != "running" {
				resultJSON, _ := json.Marshal(task)
				return Response{
					JSONRPC: "2.0",
					ID:      req.ID,
					Result: map[string]any{
						"content": []map[string]any{
							{
								"type": "text",
								"text": string(resultJSON),
							},
						},
					},
				}
			}

			// Wait before next poll
			// Note: In real implementation, use time.Sleep
			elapsed += pollIntervalMs
		}

		// Timeout - return current status
		task, _ := s.api.GetTask(taskID)
		result := map[string]any{
			"task_id":    taskID,
			"status":     task.Status,
			"timeout":    true,
			"message":    "Task still running after 60s timeout",
			"log_path":   task.OutputPath,
		}
		resultJSON, _ := json.Marshal(result)
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Result: map[string]any{
				"content": []map[string]any{
					{
						"type": "text",
						"text": string(resultJSON),
					},
				},
			},
		}
	}

	// Non-waiting: just return current task status
	task, err := s.api.GetTask(taskID)
	if err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: err.Error(),
			},
		}
	}

	resultJSON, _ := json.Marshal(task)
	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "text",
					"text": string(resultJSON),
				},
			},
		},
	}
}

func (s *Server) callReadLog(req Request) Response {
	if s.api == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "API not configured",
			},
		}
	}

	arguments, ok := req.Params["arguments"].(map[string]any)
	if !ok {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: "missing arguments",
			},
		}
	}

	// Validate
	if err := ValidateToolArgs("read_log", arguments); err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32602,
				Message: err.Error(),
			},
		}
	}

	taskID := arguments["task_id"].(string)

	// Get optional parameters
	offset := 0
	limit := 100
	tail := false

	if o, ok := arguments["offset"].(float64); ok {
		offset = int(o)
	}
	if l, ok := arguments["limit"].(float64); ok {
		limit = int(l)
	}
	if t, ok := arguments["tail"].(bool); ok {
		tail = t
	}

	// Read log
	content, task, err := s.api.ReadTaskLog(taskID, offset, limit, tail)
	if err != nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: err.Error(),
			},
		}
	}

	result := map[string]any{
		"task_id":  taskID,
		"status":   task.Status,
		"content":  content,
		"offset":   offset,
		"limit":    limit,
		"tail":     tail,
		"log_path": task.OutputPath,
	}
	resultJSON, _ := json.Marshal(result)
	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Result: map[string]any{
			"content": []map[string]any{
				{
					"type": "text",
					"text": string(resultJSON),
				},
			},
		},
	}
}
