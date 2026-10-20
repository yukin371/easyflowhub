package mcp

import (
	"encoding/json"
	"scriptmgr/internal/notes"
)

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
