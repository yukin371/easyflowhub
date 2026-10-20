package mcp

import (
	"encoding/json"
	"scriptmgr/internal/model"
)

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
