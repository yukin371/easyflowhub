package mcp

import (
	"testing"

	"scriptmgr/internal/validator"
)

func TestValidateToolArgs(t *testing.T) {
	tests := []struct {
		name     string
		toolName string
		args     map[string]any
		wantErr  bool
		errField string
		errCode  string
	}{
		{
			name:     "run_script valid args returns nil",
			toolName: "run_script",
			args: map[string]any{
				"script_id": "test-script-1",
				"async":     true,
				"dry_run":   false,
			},
			wantErr: false,
		},
		{
			name:     "run_script missing script_id returns error",
			toolName: "run_script",
			args:     map[string]any{},
			wantErr:  true,
			errField: "script_id",
			errCode:  "required",
		},
		{
			name:     "run_script non-string script_id returns error",
			toolName: "run_script",
			args: map[string]any{
				"script_id": 123,
			},
			wantErr:  true,
			errField: "script_id",
			errCode:  "type",
		},
		{
			name:     "run_script non-array args returns error",
			toolName: "run_script",
			args: map[string]any{
				"script_id": "test-script-1",
				"args":      "not-an-array",
			},
			wantErr:  true,
			errField: "args",
			errCode:  "type",
		},
		{
			name:     "run_script array items must be strings",
			toolName: "run_script",
			args: map[string]any{
				"script_id": "test-script-1",
				"args":      []any{1, "2", "3"},
			},
			wantErr:  true,
			errField: "args[0]",
			errCode:  "type",
		},
		{
			name:     "run_script non-boolean async returns error",
			toolName: "run_script",
			args: map[string]any{
				"script_id": "test-script-1",
				"async":     "not-a-bool",
			},
			wantErr:  true,
			errField: "async",
			errCode:  "type",
		},
		{
			name:     "run_script non-boolean dry_run returns error",
			toolName: "run_script",
			args: map[string]any{
				"script_id": "test-script-1",
				"dry_run":   "not-a-bool",
			},
			wantErr:  true,
			errField: "dry_run",
			errCode:  "type",
		},
		{
			name:     "list_scripts valid search returns nil",
			toolName: "list_scripts",
			args: map[string]any{
				"search": "test-search",
			},
			wantErr: false,
		},
		{
			name:     "list_scripts non-string search returns error",
			toolName: "list_scripts",
			args: map[string]any{
				"search": 123,
			},
			wantErr:  true,
			errField: "search",
			errCode:  "type",
		},
		{
			name:     "unknown tool returns nil",
			toolName: "unknown_tool",
			args:     map[string]any{},
			wantErr:  false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := ValidateToolArgs(tt.toolName, tt.args)

			if tt.wantErr {
				if err == nil {
					t.Fatalf("expected error, got nil")
				}
				errs, ok := err.(validator.ValidationErrors)
				if !ok {
					t.Fatalf("expected ValidationErrors, got %T", err)
				}
				if len(errs) == 0 {
					t.Fatalf("expected at least one validation error")
				}
				if tt.errField != "" && errs[0].Field != tt.errField {
					t.Errorf("expected error field %q, got %q", tt.errField, errs[0].Field)
				}
				if tt.errCode != "" && errs[0].Code != tt.errCode {
					t.Errorf("expected error code %q, got %q", tt.errCode, errs[0].Code)
				}
				return
			}

			if err != nil {
				t.Fatalf("expected no error, got %v", err)
			}
		})
	}
}

func TestGetToolSchemas(t *testing.T) {
	schemas := GetToolSchemas()

	if schemas == nil {
		t.Fatalf("expected non-nil map")
	}

	if len(schemas) == 0 {
		t.Fatalf("expected at least one schema")
	}

	foundList := false
	foundRun := false
	for _, schema := range schemas {
		if schema.Name == "list_scripts" {
			foundList = true
		}
		if schema.Name == "run_script" {
			foundRun = true
		}
	}
	if !foundList {
		t.Errorf("expected list_scripts in schemas")
	}
	if !foundRun {
		t.Errorf("expected run_script in schemas")
	}

	t.Run("verify schema structure", func(t *testing.T) {
		for _, schema := range schemas {
			if schema.InputSchema == nil {
				t.Errorf("expected inputSchema for %s", schema.Name)
			} else {
				if schema.InputSchema["type"] != "object" {
					t.Errorf("expected inputSchema type to be 'object', got %v", schema.InputSchema["type"])
				}
				if schema.InputSchema["properties"] == nil {
					t.Errorf("expected inputSchema properties for %s", schema.Name)
				}
			}
		}
	})
}
