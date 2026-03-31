package mcp

import (
	"fmt"

	"scriptmgr/internal/validator"
)

// ToolWithSchema represents an MCP tool with JSON Schema for input validation
type ToolWithSchema struct {
	Name        string         `json:"name"`
	Description string         `json:"description"`
	InputSchema map[string]any `json:"inputSchema"`
}

// GetToolSchemas returns all available MCP tools with their JSON Schema definitions
func GetToolSchemas() []ToolWithSchema {
	return []ToolWithSchema{
		// Script tools
		{
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
		},
		{
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
		},
		// Category management tools
		{
			Name:        "list_categories",
			Description: "List all script categories with their script counts. Use this to discover available script categories before loading them.",
			InputSchema: map[string]any{
				"type":       "object",
				"properties": map[string]any{},
			},
		},
		{
			Name:        "load_category",
			Description: "Load all scripts from a category as dynamic tools. After loading, scripts become available as script_<id> tools. Use list_categories to see available categories.",
			InputSchema: map[string]any{
				"type": "object",
				"properties": map[string]any{
					"category": map[string]any{
						"type":        "string",
						"description": "Category name to load (use 'uncategorized' for scripts without a category)",
					},
				},
				"required": []string{"category"},
			},
		},
		{
			Name:        "unload_category",
			Description: "Unload a previously loaded category to free up tool slots. Scripts from this category will no longer be available as tools.",
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
		},
		{
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
		},
		// Async task tools
		{
			Name:        "get_task_result",
			Description: "Get the result of an async script execution. Set wait=true to block until task completes. Returns task status, exit code, duration, and output summary.",
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
						"description": "Block until task completes (max 60s)",
					},
				},
				"required": []string{"task_id"},
			},
		},
		{
			Name:        "read_log",
			Description: "Read the execution log of a task. Supports pagination and tail mode for recent output.",
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
		},
		// Notes tools
		{
			Name:        "get_notes_repo",
			Description: "Get the path to the notes repository where Claude can read/write notes",
			InputSchema: map[string]any{
				"type":       "object",
				"properties": map[string]any{},
			},
		},
		{
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
		},
		{
			Name:        "sync_notes",
			Description: "Synchronize notes between DeskFlow database and file repository. Use bidirectional for full sync.",
			InputSchema: map[string]any{
				"type": "object",
				"properties": map[string]any{
					"direction": map[string]any{
						"type":        "string",
						"description": "Sync direction: bidirectional (default), db_to_file, file_to_db",
						"enum":        []string{"bidirectional", "db_to_file", "file_to_db"},
						"default":     "bidirectional",
					},
				},
			},
		},
		{
			Name:        "list_notes",
			Description: "List all notes from the file repository",
			InputSchema: map[string]any{
				"type": "object",
				"properties": map[string]any{
					"search": map[string]any{
						"type":        "string",
						"description": "Search filter for title, content or tags",
					},
				},
			},
		},
		{
			Name:        "get_note",
			Description: "Get a single note by ID from the file repository",
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
		},
		// Multi-repo management tools
		{
			Name:        "list_note_repos",
			Description: "List all configured notes repositories with their info. Shows which repo is currently active and note counts.",
			InputSchema: map[string]any{
				"type":       "object",
				"properties": map[string]any{},
			},
		},
		{
			Name:        "add_note_repo",
			Description: "Add a new notes repository. The repo must be a directory containing markdown notes.",
			InputSchema: map[string]any{
				"type": "object",
				"properties": map[string]any{
					"id": map[string]any{
						"type":        "string",
						"description": "Unique identifier for this repo (e.g., 'work', 'personal')",
					},
					"name": map[string]any{
						"type":        "string",
						"description": "Display name for this repo",
					},
					"path": map[string]any{
						"type":        "string",
						"description": "Absolute path to the notes directory",
					},
				},
				"required": []string{"id", "name", "path"},
			},
		},
		{
			Name:        "remove_note_repo",
			Description: "Remove a notes repository by ID. Cannot remove the last repo.",
			InputSchema: map[string]any{
				"type": "object",
				"properties": map[string]any{
					"id": map[string]any{
						"type":        "string",
						"description": "ID of the repo to remove",
					},
				},
				"required": []string{"id"},
			},
		},
		{
			Name:        "set_current_note_repo",
			Description: "Switch the active notes repository. All notes operations will use this repo.",
			InputSchema: map[string]any{
				"type": "object",
				"properties": map[string]any{
					"id": map[string]any{
						"type":        "string",
						"description": "ID of the repo to activate",
					},
				},
				"required": []string{"id"},
			},
		},
	}
}

// ValidateToolArgs validates arguments for a given tool name
// Returns ValidationErrors with field details on failure, nil on success
func ValidateToolArgs(toolName string, args map[string]any) error {
	var errs validator.ValidationErrors

	switch toolName {
	case "run_script":
		// script_id is required and must be a string
		scriptID, exists := args["script_id"]
		if !exists {
			errs = append(errs, validator.ValidationError{
				Field:   "script_id",
				Code:    "required",
				Message: "script_id is required",
			})
		} else {
			if _, ok := scriptID.(string); !ok {
				errs = append(errs, validator.ValidationError{
					Field:   "script_id",
					Code:    "type",
					Message: "script_id must be a string",
					Value:   scriptID,
				})
			}
		}

		// args is optional but must be array of strings if present
		if argsVal, exists := args["args"]; exists && argsVal != nil {
			if argsArr, ok := argsVal.([]any); ok {
				for i, item := range argsArr {
					if _, ok := item.(string); !ok {
						errs = append(errs, validator.ValidationError{
							Field:   fmt.Sprintf("args[%d]", i),
							Code:    "type",
							Message: "array items must be strings",
							Value:   item,
						})
					}
				}
			} else {
				errs = append(errs, validator.ValidationError{
					Field:   "args",
					Code:    "type",
					Message: "args must be an array",
					Value:   argsVal,
				})
			}
		}

		// async is optional but must be boolean if present
		if asyncVal, exists := args["async"]; exists && asyncVal != nil {
			if _, ok := asyncVal.(bool); !ok {
				errs = append(errs, validator.ValidationError{
					Field:   "async",
					Code:    "type",
					Message: "async must be a boolean",
					Value:   asyncVal,
				})
			}
		}

		// dry_run is optional but must be boolean if present
		if dryRunVal, exists := args["dry_run"]; exists && dryRunVal != nil {
			if _, ok := dryRunVal.(bool); !ok {
				errs = append(errs, validator.ValidationError{
					Field:   "dry_run",
					Code:    "type",
					Message: "dry_run must be a boolean",
					Value:   dryRunVal,
				})
			}
		}

	case "list_scripts":
		// search is optional but must be string if present
		if searchVal, exists := args["search"]; exists && searchVal != nil {
			if _, ok := searchVal.(string); !ok {
				errs = append(errs, validator.ValidationError{
					Field:   "search",
					Code:    "type",
					Message: "search must be a string",
					Value:   searchVal,
				})
			}
		}

	// Category management tools
	case "list_categories":
		// No arguments required

	case "load_category":
		// category is required and must be a string
		categoryVal, exists := args["category"]
		if !exists {
			errs = append(errs, validator.ValidationError{
				Field:   "category",
				Code:    "required",
				Message: "category is required",
			})
		} else if _, ok := categoryVal.(string); !ok {
			errs = append(errs, validator.ValidationError{
				Field:   "category",
				Code:    "type",
				Message: "category must be a string",
				Value:   categoryVal,
			})
		}

	case "unload_category":
		// category is required and must be a string
		categoryVal, exists := args["category"]
		if !exists {
			errs = append(errs, validator.ValidationError{
				Field:   "category",
				Code:    "required",
				Message: "category is required",
			})
		} else if _, ok := categoryVal.(string); !ok {
			errs = append(errs, validator.ValidationError{
				Field:   "category",
				Code:    "type",
				Message: "category must be a string",
				Value:   categoryVal,
			})
		}

	case "search_scripts":
		// keyword is optional but must be string if present
		if keywordVal, exists := args["keyword"]; exists && keywordVal != nil {
			if _, ok := keywordVal.(string); !ok {
				errs = append(errs, validator.ValidationError{
					Field:   "keyword",
					Code:    "type",
					Message: "keyword must be a string",
					Value:   keywordVal,
				})
			}
		}

	// Async task tools
	case "get_task_result":
		// task_id is required and must be a string
		taskIDVal, exists := args["task_id"]
		if !exists {
			errs = append(errs, validator.ValidationError{
				Field:   "task_id",
				Code:    "required",
				Message: "task_id is required",
			})
		} else if _, ok := taskIDVal.(string); !ok {
			errs = append(errs, validator.ValidationError{
				Field:   "task_id",
				Code:    "type",
				Message: "task_id must be a string",
				Value:   taskIDVal,
			})
		}

		// wait is optional but must be boolean if present
		if waitVal, exists := args["wait"]; exists && waitVal != nil {
			if _, ok := waitVal.(bool); !ok {
				errs = append(errs, validator.ValidationError{
					Field:   "wait",
					Code:    "type",
					Message: "wait must be a boolean",
					Value:   waitVal,
				})
			}
		}

	case "read_log":
		// task_id is required and must be a string
		taskIDVal, exists := args["task_id"]
		if !exists {
			errs = append(errs, validator.ValidationError{
				Field:   "task_id",
				Code:    "required",
				Message: "task_id is required",
			})
		} else if _, ok := taskIDVal.(string); !ok {
			errs = append(errs, validator.ValidationError{
				Field:   "task_id",
				Code:    "type",
				Message: "task_id must be a string",
				Value:   taskIDVal,
			})
		}

		// offset, limit are optional but must be integers if present
		if offsetVal, exists := args["offset"]; exists && offsetVal != nil {
			if _, ok := offsetVal.(float64); !ok {
				errs = append(errs, validator.ValidationError{
					Field:   "offset",
					Code:    "type",
					Message: "offset must be an integer",
					Value:   offsetVal,
				})
			}
		}
		if limitVal, exists := args["limit"]; exists && limitVal != nil {
			if _, ok := limitVal.(float64); !ok {
				errs = append(errs, validator.ValidationError{
					Field:   "limit",
					Code:    "type",
					Message: "limit must be an integer",
					Value:   limitVal,
				})
			}
		}

		// tail is optional but must be boolean if present
		if tailVal, exists := args["tail"]; exists && tailVal != nil {
			if _, ok := tailVal.(bool); !ok {
				errs = append(errs, validator.ValidationError{
					Field:   "tail",
					Code:    "type",
					Message: "tail must be a boolean",
					Value:   tailVal,
				})
			}
		}

	// Notes tools
	case "set_notes_repo":
		// path is required and must be a string
		pathVal, exists := args["path"]
		if !exists {
			errs = append(errs, validator.ValidationError{
				Field:   "path",
				Code:    "required",
				Message: "path is required",
			})
		} else if _, ok := pathVal.(string); !ok {
			errs = append(errs, validator.ValidationError{
				Field:   "path",
				Code:    "type",
				Message: "path must be a string",
				Value:   pathVal,
			})
		}

	case "sync_notes":
		// direction is optional but must be valid if present
		if dirVal, exists := args["direction"]; exists && dirVal != nil {
			dir, ok := dirVal.(string)
			if !ok {
				errs = append(errs, validator.ValidationError{
					Field:   "direction",
					Code:    "type",
					Message: "direction must be a string",
					Value:   dirVal,
				})
			} else if dir != "bidirectional" && dir != "db_to_file" && dir != "file_to_db" {
				errs = append(errs, validator.ValidationError{
					Field:   "direction",
					Code:    "enum",
					Message: "direction must be one of: bidirectional, db_to_file, file_to_db",
					Value:   dirVal,
				})
			}
		}

	case "list_notes":
		// search is optional but must be string if present
		if searchVal, exists := args["search"]; exists && searchVal != nil {
			if _, ok := searchVal.(string); !ok {
				errs = append(errs, validator.ValidationError{
					Field:   "search",
					Code:    "type",
					Message: "search must be a string",
					Value:   searchVal,
				})
			}
		}

	case "get_note":
		// note_id is required and must be a string
		noteIDVal, exists := args["note_id"]
		if !exists {
			errs = append(errs, validator.ValidationError{
				Field:   "note_id",
				Code:    "required",
				Message: "note_id is required",
			})
		} else if _, ok := noteIDVal.(string); !ok {
			errs = append(errs, validator.ValidationError{
				Field:   "note_id",
				Code:    "type",
				Message: "note_id must be a string",
				Value:   noteIDVal,
			})
		}

	case "list_note_repos":
		// No arguments required

	case "add_note_repo":
		// id, name, path are required and must be strings
		for _, field := range []string{"id", "name", "path"} {
			val, exists := args[field]
			if !exists {
				errs = append(errs, validator.ValidationError{
					Field:   field,
					Code:    "required",
					Message: field + " is required",
				})
			} else if _, ok := val.(string); !ok {
				errs = append(errs, validator.ValidationError{
					Field:   field,
					Code:    "type",
					Message: field + " must be a string",
					Value:   val,
				})
			}
		}

	case "remove_note_repo":
		// id is required and must be a string
		idVal, exists := args["id"]
		if !exists {
			errs = append(errs, validator.ValidationError{
				Field:   "id",
				Code:    "required",
				Message: "id is required",
			})
		} else if _, ok := idVal.(string); !ok {
			errs = append(errs, validator.ValidationError{
				Field:   "id",
				Code:    "type",
				Message: "id must be a string",
				Value:   idVal,
			})
		}

	case "set_current_note_repo":
		// id is required and must be a string
		idVal, exists := args["id"]
		if !exists {
			errs = append(errs, validator.ValidationError{
				Field:   "id",
				Code:    "required",
				Message: "id is required",
			})
		} else if _, ok := idVal.(string); !ok {
			errs = append(errs, validator.ValidationError{
				Field:   "id",
				Code:    "type",
				Message: "id must be a string",
				Value:   idVal,
			})
		}
	}

	if len(errs) > 0 {
		return errs
	}
	return nil
}
