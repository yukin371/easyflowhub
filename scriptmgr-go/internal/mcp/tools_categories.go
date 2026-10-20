package mcp

import (
	"encoding/json"
	"fmt"
)

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
