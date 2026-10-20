package mcp

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
			"name":   cat.Name,
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

	// The router is the single dispatch owner for static and dynamic tools
	// (F11); a tool it does not handle is genuinely unknown. Reaching here
	// with a nil router means the server was never wired with an API.
	if s.router == nil {
		return Response{
			JSONRPC: "2.0",
			ID:      req.ID,
			Error: &ResponseError{
				Code:    -32603,
				Message: "API not configured",
			},
		}
	}

	return Response{
		JSONRPC: "2.0",
		ID:      req.ID,
		Error: &ResponseError{
			Code:    -32602,
			Message: "unknown tool: " + toolName,
		},
	}
}
