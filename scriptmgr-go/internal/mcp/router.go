package mcp

import (
	"encoding/json"
	"fmt"
	"regexp"
	"sort"
	"strings"
	"sync"
	"time"

	"scriptmgr/internal/model"
)

// RouterConfig holds configuration for the tool router
type RouterConfig struct {
	MaxCategories int // Maximum number of categories that can be loaded
}

// DefaultRouterConfig returns default configuration
func DefaultRouterConfig() RouterConfig {
	return RouterConfig{
		MaxCategories: 20,
	}
}

// CategoryInfo represents information about a script category
type CategoryInfo struct {
	Name   string `json:"name"`
	Count  int    `json:"count"`
	Loaded bool   `json:"loaded"`
}

// LoadedCategory represents a category that has been loaded with its scripts
type LoadedCategory struct {
	Name       string
	Scripts    []model.ScriptRecord
	LoadedAt   time.Time
	LastUsedAt time.Time
	ToolNames  []string
}

// ToolHandler is a function that handles a tool call
type ToolHandler func(req Request) Response

// ToolRouter manages static and dynamic MCP tools
type ToolRouter struct {
	mu sync.RWMutex

	// API for script operations
	api ScriptAPI

	// Configuration
	config RouterConfig

	// Static tools (always available)
	staticTools   map[string]ToolWithSchema
	staticHandlers map[string]ToolHandler

	// Dynamic tools (loaded by category)
	loadedCategories map[string]*LoadedCategory
	toolToCategory   map[string]string // toolName -> categoryName
	dynamicSchemas   map[string]ToolWithSchema
	dynamicHandlers  map[string]ToolHandler
}

// NewToolRouter creates a new tool router
func NewToolRouter(api ScriptAPI, config RouterConfig) *ToolRouter {
	return &ToolRouter{
		api:              api,
		config:           config,
		staticTools:      make(map[string]ToolWithSchema),
		staticHandlers:   make(map[string]ToolHandler),
		loadedCategories: make(map[string]*LoadedCategory),
		toolToCategory:   make(map[string]string),
		dynamicSchemas:   make(map[string]ToolWithSchema),
		dynamicHandlers:  make(map[string]ToolHandler),
	}
}

// RegisterStatic registers a static tool that is always available
func (r *ToolRouter) RegisterStatic(name string, schema ToolWithSchema, handler ToolHandler) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.staticTools[name] = schema
	r.staticHandlers[name] = handler
}

// GetDiscoveryTools returns only static/discovery tools (no dynamic script tools)
// This enables progressive disclosure - script tools only appear after load_category
func (r *ToolRouter) GetDiscoveryTools() []ToolWithSchema {
	r.mu.RLock()
	defer r.mu.RUnlock()

	// Convert map to slice
	tools := make([]ToolWithSchema, 0, len(r.staticTools))
	for _, schema := range r.staticTools {
		tools = append(tools, schema)
	}

	// Sort by name for consistent ordering
	sort.Slice(tools, func(i, j int) bool {
		return tools[i].Name < tools[j].Name
	})

	return tools
}

// GetToolSchemas returns all available tool schemas (static + dynamic)
func (r *ToolRouter) GetToolSchemas() []ToolWithSchema {
	r.mu.RLock()
	defer r.mu.RUnlock()

	// Count total tools
	total := len(r.staticTools) + len(r.dynamicSchemas)
	tools := make([]ToolWithSchema, 0, total)

	// Add static tools
	for _, schema := range r.staticTools {
		tools = append(tools, schema)
	}

	// Add dynamic tools
	for _, schema := range r.dynamicSchemas {
		tools = append(tools, schema)
	}

	// Sort by name for consistent ordering
	sort.Slice(tools, func(i, j int) bool {
		return tools[i].Name < tools[j].Name
	})

	return tools
}

// HandleTool routes a tool call to the appropriate handler
// Returns (response, handled) where handled indicates if the tool was found
func (r *ToolRouter) HandleTool(toolName string, req Request) (Response, bool) {
	r.mu.RLock()

	// Check static handlers first
	if handler, ok := r.staticHandlers[toolName]; ok {
		r.mu.RUnlock()
		return handler(req), true
	}

	// Check dynamic handlers
	if handler, ok := r.dynamicHandlers[toolName]; ok {
		// Update last used time for the category
		if categoryName, ok := r.toolToCategory[toolName]; ok {
			if cat, ok := r.loadedCategories[categoryName]; ok {
				cat.LastUsedAt = time.Now()
			}
		}
		r.mu.RUnlock()
		return handler(req), true
	}

	r.mu.RUnlock()
	return Response{}, false
}

// ListCategories returns all script categories with their counts
func (r *ToolRouter) ListCategories() ([]CategoryInfo, error) {
	scripts, _, err := r.api.ListScripts("")
	if err != nil {
		return nil, fmt.Errorf("failed to list scripts: %w", err)
	}

	// Count scripts by category
	categoryMap := make(map[string]int)
	for _, script := range scripts {
		cat := script.Category
		if cat == "" {
			cat = "uncategorized"
		}
		categoryMap[cat]++
	}

	r.mu.RLock()
	defer r.mu.RUnlock()

	// Build category info list
	var categories []CategoryInfo
	for name, count := range categoryMap {
		_, loaded := r.loadedCategories[name]
		categories = append(categories, CategoryInfo{
			Name:   name,
			Count:  count,
			Loaded: loaded,
		})
	}

	// Sort by name
	sort.Slice(categories, func(i, j int) bool {
		return categories[i].Name < categories[j].Name
	})

	return categories, nil
}

// LoadCategory loads all scripts from a category as dynamic tools
func (r *ToolRouter) LoadCategory(categoryName string) (*LoadCategoryResult, error) {
	// Normalize category name
	if categoryName == "" {
		categoryName = "uncategorized"
	}

	// Get scripts in this category
	scripts, _, err := r.api.ListScripts("")
	if err != nil {
		return nil, fmt.Errorf("failed to list scripts: %w", err)
	}

	var categoryScripts []model.ScriptRecord
	for _, script := range scripts {
		cat := script.Category
		if cat == "" {
			cat = "uncategorized"
		}
		if strings.EqualFold(cat, categoryName) {
			categoryScripts = append(categoryScripts, script)
		}
	}

	if len(categoryScripts) == 0 {
		return nil, fmt.Errorf("category '%s' not found or has no scripts", categoryName)
	}

	r.mu.Lock()
	defer r.mu.Unlock()

	// Check if already loaded
	if existing, ok := r.loadedCategories[categoryName]; ok {
		existing.LastUsedAt = time.Now()
		return &LoadCategoryResult{
			Category:    categoryName,
			ScriptCount: len(existing.Scripts),
			AlreadyLoaded: true,
		}, nil
	}

	// Check if we need to evict a category (LRU)
	var evicted []string
	if len(r.loadedCategories) >= r.config.MaxCategories {
		evicted = r.evictLRULocked()
	}

	// Register tools for each script
	toolNames := make([]string, 0, len(categoryScripts))
	now := time.Now()

	for _, script := range categoryScripts {
		toolName := sanitizeToolName(script.ID)
		schema := buildScriptToolSchema(script)
		handler := r.createScriptHandler(script)

		r.dynamicSchemas[toolName] = schema
		r.dynamicHandlers[toolName] = handler
		r.toolToCategory[toolName] = categoryName
		toolNames = append(toolNames, toolName)
	}

	// Store the loaded category
	r.loadedCategories[categoryName] = &LoadedCategory{
		Name:       categoryName,
		Scripts:    categoryScripts,
		LoadedAt:   now,
		LastUsedAt: now,
		ToolNames:  toolNames,
	}

	return &LoadCategoryResult{
		Category:       categoryName,
		ScriptCount:    len(categoryScripts),
		AlreadyLoaded:  false,
		EvictedCategories: evicted,
		TotalTools:     len(r.staticTools) + len(r.dynamicSchemas),
	}, nil
}

// LoadCategoryResult contains the result of loading a category
type LoadCategoryResult struct {
	Category          string   `json:"category"`
	ScriptCount       int      `json:"script_count"`
	AlreadyLoaded     bool     `json:"already_loaded"`
	EvictedCategories []string `json:"evicted_categories,omitempty"`
	TotalTools        int      `json:"total_tools"`
}

// UnloadCategory unloads a previously loaded category
func (r *ToolRouter) UnloadCategory(categoryName string) error {
	if categoryName == "" {
		categoryName = "uncategorized"
	}

	r.mu.Lock()
	defer r.mu.Unlock()

	cat, ok := r.loadedCategories[categoryName]
	if !ok {
		// Not loaded, nothing to do
		return nil
	}

	// Remove all tools from this category
	for _, toolName := range cat.ToolNames {
		delete(r.dynamicSchemas, toolName)
		delete(r.dynamicHandlers, toolName)
		delete(r.toolToCategory, toolName)
	}

	// Remove the category
	delete(r.loadedCategories, categoryName)

	return nil
}

// IsCategoryLoaded checks if a category is currently loaded
func (r *ToolRouter) IsCategoryLoaded(categoryName string) bool {
	if categoryName == "" {
		categoryName = "uncategorized"
	}
	r.mu.RLock()
	defer r.mu.RUnlock()
	_, ok := r.loadedCategories[categoryName]
	return ok
}

// GetLoadedCategories returns the names of all loaded categories
func (r *ToolRouter) GetLoadedCategories() []string {
	r.mu.RLock()
	defer r.mu.RUnlock()

	names := make([]string, 0, len(r.loadedCategories))
	for name := range r.loadedCategories {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

// SearchScripts searches for scripts by keyword
func (r *ToolRouter) SearchScripts(keyword string) ([]model.ScriptRecord, error) {
	scripts, _, err := r.api.ListScripts(keyword)
	if err != nil {
		return nil, fmt.Errorf("failed to search scripts: %w", err)
	}
	return scripts, nil
}

// evictLRULocked evicts the least recently used category
// Must be called with lock held
func (r *ToolRouter) evictLRULocked() []string {
	if len(r.loadedCategories) == 0 {
		return nil
	}

	// Find the oldest category by LastUsedAt
	var oldest *LoadedCategory
	for _, cat := range r.loadedCategories {
		if oldest == nil || cat.LastUsedAt.Before(oldest.LastUsedAt) {
			oldest = cat
		}
	}

	if oldest == nil {
		return nil
	}

	// Remove all tools from this category
	for _, toolName := range oldest.ToolNames {
		delete(r.dynamicSchemas, toolName)
		delete(r.dynamicHandlers, toolName)
		delete(r.toolToCategory, toolName)
	}

	// Remove the category
	delete(r.loadedCategories, oldest.Name)

	return []string{oldest.Name}
}

// createScriptHandler creates a handler function for a script tool
func (r *ToolRouter) createScriptHandler(script model.ScriptRecord) ToolHandler {
	return func(req Request) Response {
		// Extract arguments
		arguments, ok := req.Params["arguments"].(map[string]any)
		if !ok {
			arguments = make(map[string]any)
		}

		// Build command line args from script parameters
		var args []string
		for _, param := range script.Parameters {
			if val, exists := arguments[param.Name]; exists && val != nil {
				args = append(args, fmt.Sprintf("%v", val))
			} else if param.Default != "" {
				args = append(args, param.Default)
			}
		}

		// Also check for explicit args array
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

		// Call the API
		result, err := r.api.RunScript(script.ID, args, opts)
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

		// Serialize result
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
}

// sanitizeToolName converts a script ID to a valid tool name
func sanitizeToolName(scriptID string) string {
	// Convert to lowercase
	sanitized := strings.ToLower(scriptID)

	// Replace spaces and hyphens with underscores
	sanitized = strings.ReplaceAll(sanitized, " ", "_")
	sanitized = strings.ReplaceAll(sanitized, "-", "_")

	// Remove any non-alphanumeric characters except underscore
	reg := regexp.MustCompile(`[^a-z0-9_]`)
	sanitized = reg.ReplaceAllString(sanitized, "_")

	// Add prefix
	return "script_" + sanitized
}

// buildScriptToolSchema generates a tool schema from a script record
func buildScriptToolSchema(script model.ScriptRecord) ToolWithSchema {
	properties := make(map[string]any)
	required := []string{}

	// Add script parameters to schema
	for _, param := range script.Parameters {
		prop := map[string]any{
			"type":        mapParamType(param.Type),
			"description": buildParamDescription(param),
		}
		if param.Default != "" {
			prop["default"] = param.Default
		}
		properties[param.Name] = prop

		if param.Required {
			required = append(required, param.Name)
		}
	}

	// Add common parameters
	properties["args"] = map[string]any{
		"type":        "array",
		"items":       map[string]any{"type": "string"},
		"description": "Additional command line arguments",
	}

	properties["async"] = map[string]any{
		"type":        "boolean",
		"default":     true,
		"description": "Run asynchronously (returns task_id for status polling)",
	}

	properties["dry_run"] = map[string]any{
		"type":        "boolean",
		"default":     false,
		"description": "Preview command without executing",
	}

	// Build description
	description := buildScriptDescription(script)

	return ToolWithSchema{
		Name:        sanitizeToolName(script.ID),
		Description: description,
		InputSchema: map[string]any{
			"type":       "object",
			"properties": properties,
			"required":   required,
		},
	}
}

// mapParamType converts a script parameter type to JSON Schema type
func mapParamType(paramType string) string {
	switch strings.ToLower(paramType) {
	case "int", "integer":
		return "integer"
	case "float", "number", "double":
		return "number"
	case "bool", "boolean":
		return "boolean"
	case "array", "list":
		return "array"
	default:
		return "string"
	}
}

// buildParamDescription creates a description for a parameter
func buildParamDescription(param model.ScriptParameter) string {
	if param.Description != "" {
		if param.Label != "" {
			return param.Label + ": " + param.Description
		}
		return param.Description
	}
	if param.Label != "" {
		return param.Label
	}
	return param.Name
}

// buildScriptDescription creates a description for a script tool
func buildScriptDescription(script model.ScriptRecord) string {
	var parts []string

	// Use name as base
	if script.Name != "" {
		parts = append(parts, script.Name)
	} else {
		parts = append(parts, script.ID)
	}

	// Add description if available
	if script.Description != "" {
		// Truncate long descriptions
		desc := script.Description
		if len(desc) > 150 {
			desc = desc[:147] + "..."
		}
		parts = append(parts, "-", desc)
	}

	// Add parameter count
	if len(script.Parameters) > 0 {
		parts = append(parts, fmt.Sprintf("(%d params)", len(script.Parameters)))
	}

	return strings.Join(parts, " ")
}
