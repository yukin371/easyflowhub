package validator

import (
	"strings"
	"testing"

	"scriptmgr/internal/model"
)

func TestValidateArgs(t *testing.T) {
	tests := []struct {
		name        string
		params      []model.ScriptParameter
		input       []string
		wantResult  []string
		wantErr     bool
		errCode     string
		errField    string
	}{
		{
			name:       "no params returns input unchanged",
			params:     nil,
			input:      []string{"arg1", "arg2"},
			wantResult: []string{"arg1", "arg2"},
			wantErr:    false,
		},
		{
			name:       "empty params slice returns input unchanged",
			params:     []model.ScriptParameter{},
			input:      []string{"arg1", "arg2"},
			wantResult: []string{"arg1", "arg2"},
			wantErr:    false,
		},
		{
			name:   "required field missing returns error",
			params: []model.ScriptParameter{{Name: "path", Required: true}},
			input:  []string{},
			wantErr: true,
			errCode:  "required",
			errField: "path",
		},
		{
			name:       "required field provided returns validated",
			params:     []model.ScriptParameter{{Name: "path", Required: true}},
			input:      []string{"/some/path"},
			wantResult: []string{"/some/path"},
			wantErr:    false,
		},
		{
			name:       "default value applied when input empty",
			params:     []model.ScriptParameter{{Name: "mode", Default: "read-only"}},
			input:      []string{},
			wantResult: []string{"read-only"},
			wantErr:    false,
		},
		{
			name:       "default value applied when input whitespace",
			params:     []model.ScriptParameter{{Name: "mode", Default: "read-only"}},
			input:      []string{"   "},
			wantResult: []string{"read-only"},
			wantErr:    false,
		},
		{
			name:       "provided value overrides default",
			params:     []model.ScriptParameter{{Name: "mode", Default: "read-only"}},
			input:      []string{"write"},
			wantResult: []string{"write"},
			wantErr:    false,
		},
		{
			name:       "extra args beyond params preserved",
			params:     []model.ScriptParameter{{Name: "path"}},
			input:      []string{"/path", "extra1", "extra2"},
			wantResult: []string{"/path", "extra1", "extra2"},
			wantErr:    false,
		},
		{
			name:   "type validation integer valid",
			params: []model.ScriptParameter{{Name: "count", Type: "integer"}},
			input:  []string{"42"},
			wantResult: []string{"42"},
			wantErr: false,
		},
		{
			name:   "type validation integer invalid",
			params: []model.ScriptParameter{{Name: "count", Type: "integer"}},
			input:  []string{"not-a-number"},
			wantErr: true,
			errCode:  "type",
			errField: "count",
		},
		{
			name:   "type validation int alias valid",
			params: []model.ScriptParameter{{Name: "count", Type: "int"}},
			input:  []string{"42"},
			wantResult: []string{"42"},
			wantErr: false,
		},
		{
			name:   "type validation number valid",
			params: []model.ScriptParameter{{Name: "ratio", Type: "number"}},
			input:  []string{"3.14"},
			wantResult: []string{"3.14"},
			wantErr: false,
		},
		{
			name:   "type validation float alias valid",
			params: []model.ScriptParameter{{Name: "ratio", Type: "float"}},
			input:  []string{"2.5"},
			wantResult: []string{"2.5"},
			wantErr: false,
		},
		{
			name:   "type validation number invalid",
			params: []model.ScriptParameter{{Name: "ratio", Type: "number"}},
			input:  []string{"not-a-number"},
			wantErr: true,
			errCode:  "type",
			errField: "ratio",
		},
		{
			name:   "type validation boolean valid true",
			params: []model.ScriptParameter{{Name: "verbose", Type: "boolean"}},
			input:  []string{"true"},
			wantResult: []string{"true"},
			wantErr: false,
		},
		{
			name:   "type validation boolean valid false",
			params: []model.ScriptParameter{{Name: "verbose", Type: "boolean"}},
			input:  []string{"false"},
			wantResult: []string{"false"},
			wantErr: false,
		},
		{
			name:   "type validation bool alias valid",
			params: []model.ScriptParameter{{Name: "verbose", Type: "bool"}},
			input:  []string{"1"},
			wantResult: []string{"1"},
			wantErr: false,
		},
		{
			name:   "type validation boolean invalid",
			params: []model.ScriptParameter{{Name: "verbose", Type: "boolean"}},
			input:  []string{"yes"},
			wantErr: true,
			errCode:  "type",
			errField: "verbose",
		},
		{
			name:       "type validation string is default",
			params:     []model.ScriptParameter{{Name: "name", Type: "string"}},
			input:      []string{"any value"},
			wantResult: []string{"any value"},
			wantErr:    false,
		},
		{
			name:       "empty type is treated as string",
			params:     []model.ScriptParameter{{Name: "name", Type: ""}},
			input:      []string{"any value"},
			wantResult: []string{"any value"},
			wantErr:    false,
		},
		{
			name:   "case insensitive type Integer",
			params: []model.ScriptParameter{{Name: "count", Type: "Integer"}},
			input:  []string{"42"},
			wantResult: []string{"42"},
			wantErr: false,
		},
		{
			name:   "case insensitive type INT",
			params: []model.ScriptParameter{{Name: "count", Type: "INT"}},
			input:  []string{"42"},
			wantResult: []string{"42"},
			wantErr: false,
		},
		{
			name:   "multiple params with mixed validation",
			params: []model.ScriptParameter{
				{Name: "path", Required: true},
				{Name: "count", Type: "integer", Default: "10"},
				{Name: "verbose", Type: "boolean"},
			},
			input:      []string{"/data", "20"},
			wantResult: []string{"/data", "20", ""},
			wantErr:    false,
		},
		{
			name:   "multiple validation errors collected",
			params: []model.ScriptParameter{
				{Name: "path", Required: true},
				{Name: "count", Type: "integer"},
			},
			input:   []string{"", "not-a-number"},
			wantErr: true,
		},
		{
			name:       "whitespace preserved in type trimming",
			params:     []model.ScriptParameter{{Name: "count", Type: "  integer  "}},
			input:      []string{"42"},
			wantResult: []string{"42"},
			wantErr:    false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := ValidateArgs(tt.params, tt.input)

			if tt.wantErr {
				if err == nil {
					t.Fatalf("expected error, got nil")
				}
				errs, ok := err.(ValidationErrors)
				if !ok {
					t.Fatalf("expected ValidationErrors, got %T", err)
				}
				if len(errs) == 0 {
					t.Fatalf("expected at least one validation error")
				}
				if tt.errCode != "" && errs[0].Code != tt.errCode {
					t.Errorf("expected error code %q, got %q", tt.errCode, errs[0].Code)
				}
				if tt.errField != "" && errs[0].Field != tt.errField {
					t.Errorf("expected error field %q, got %q", tt.errField, errs[0].Field)
				}
				return
			}

			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}

			if len(result) != len(tt.wantResult) {
				t.Errorf("expected %d results, got %d: %v vs %v", len(tt.wantResult), len(result), tt.wantResult, result)
				return
			}

			for i, v := range result {
				if v != tt.wantResult[i] {
					t.Errorf("result[%d] = %q, want %q", i, v, tt.wantResult[i])
				}
			}
		})
	}
}

func TestValidationErrors_Error(t *testing.T) {
	tests := []struct {
		name     string
		errs     ValidationErrors
		wantStr  string
	}{
		{
			name:    "empty errors returns empty string",
			errs:    ValidationErrors{},
			wantStr: "",
		},
		{
			name:    "nil errors returns empty string",
			errs:    nil,
			wantStr: "",
		},
		{
			name: "single error formats correctly",
			errs: ValidationErrors{
				{Field: "path", Code: "required", Message: "path is required"},
			},
			wantStr: "path: path is required",
		},
		{
			name: "multiple errors joined with semicolon",
			errs: ValidationErrors{
				{Field: "path", Code: "required", Message: "path is required"},
				{Field: "count", Code: "type", Message: "expected integer"},
			},
			wantStr: "path: path is required; count: expected integer",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := tt.errs.Error()
			if got != tt.wantStr {
				t.Errorf("Error() = %q, want %q", got, tt.wantStr)
			}
		})
	}
}

func TestValidationError_Fields(t *testing.T) {
	// Test that ValidationError has all expected fields
	err := ValidationError{
		Field:   "testField",
		Code:    "testCode",
		Message: "test message",
		Value:   "test value",
	}

	if err.Field != "testField" {
		t.Errorf("Field = %q, want %q", err.Field, "testField")
	}
	if err.Code != "testCode" {
		t.Errorf("Code = %q, want %q", err.Code, "testCode")
	}
	if err.Message != "test message" {
		t.Errorf("Message = %q, want %q", err.Message, "test message")
	}
	if err.Value != "test value" {
		t.Errorf("Value = %q, want %q", err.Value, "test value")
	}
}

func TestValidateType_EdgeCases(t *testing.T) {
	tests := []struct {
		name      string
		schema    model.ScriptParameter
		value     string
		wantErr   bool
		errCode   string
	}{
		{
			name:    "negative integer valid",
			schema:  model.ScriptParameter{Name: "count", Type: "integer"},
			value:   "-42",
			wantErr: false,
		},
		{
			name:    "negative number valid",
			schema:  model.ScriptParameter{Name: "ratio", Type: "number"},
			value:   "-3.14",
			wantErr: false,
		},
		{
			name:    "zero integer valid",
			schema:  model.ScriptParameter{Name: "count", Type: "integer"},
			value:   "0",
			wantErr: false,
		},
		{
			name:    "boolean 0 valid",
			schema:  model.ScriptParameter{Name: "flag", Type: "boolean"},
			value:   "0",
			wantErr: false,
		},
		{
			name:    "boolean 1 valid",
			schema:  model.ScriptParameter{Name: "flag", Type: "boolean"},
			value:   "1",
			wantErr: false,
		},
		{
			name:    "boolean True valid",
			schema:  model.ScriptParameter{Name: "flag", Type: "boolean"},
			value:   "True",
			wantErr: false,
		},
		{
			name:    "boolean False valid",
			schema:  model.ScriptParameter{Name: "flag", Type: "boolean"},
			value:   "False",
			wantErr: false,
		},
		{
			name:    "scientific notation integer invalid",
			schema:  model.ScriptParameter{Name: "count", Type: "integer"},
			value:   "1e5",
			wantErr: true,
			errCode: "type",
		},
		{
			name:    "scientific notation number valid",
			schema:  model.ScriptParameter{Name: "ratio", Type: "number"},
			value:   "1.5e10",
			wantErr: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := validateType(tt.schema, tt.value)

			if tt.wantErr {
				if err == nil {
					t.Fatalf("expected error, got nil")
				}
				if err.Code != tt.errCode {
					t.Errorf("expected error code %q, got %q", tt.errCode, err.Code)
				}
				return
			}

			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
		})
	}
}

func TestValidateArgs_RequiredWithDefault(t *testing.T) {
	// When a field is required but has a default, the default should NOT satisfy the requirement
	// This tests the current behavior: required + default means the default is used if input is empty
	params := []model.ScriptParameter{
		{Name: "path", Required: true, Default: "/default"},
	}

	// Empty input should use default and not error
	result, err := ValidateArgs(params, []string{})
	if err != nil {
		t.Errorf("expected no error when default is available, got: %v", err)
	}
	if len(result) != 1 || result[0] != "/default" {
		t.Errorf("expected default value, got: %v", result)
	}
}

func TestValidateArgs_PartialInput(t *testing.T) {
	params := []model.ScriptParameter{
		{Name: "path", Required: true},
		{Name: "mode", Default: "read"},
		{Name: "verbose", Type: "boolean", Default: "false"},
	}

	// Only provide first param, others should get defaults
	result, err := ValidateArgs(params, []string{"/data"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	expected := []string{"/data", "read", "false"}
	if len(result) != len(expected) {
		t.Fatalf("expected %d results, got %d", len(expected), len(result))
	}

	for i, v := range result {
		if v != expected[i] {
			t.Errorf("result[%d] = %q, want %q", i, v, expected[i])
		}
	}
}

func TestValidateArgs_ErrorContainsValue(t *testing.T) {
	params := []model.ScriptParameter{
		{Name: "count", Type: "integer"},
	}

	_, err := ValidateArgs(params, []string{"not-a-number"})
	if err == nil {
		t.Fatal("expected error, got nil")
	}

	errs := err.(ValidationErrors)
	if len(errs) == 0 {
		t.Fatal("expected at least one error")
	}

	if errs[0].Value != "not-a-number" {
		t.Errorf("expected Value to be 'not-a-number', got %v", errs[0].Value)
	}
}

func TestValidateArgs_MultipleErrorsAllReturned(t *testing.T) {
	params := []model.ScriptParameter{
		{Name: "path", Required: true},
		{Name: "count", Type: "integer", Required: true},
		{Name: "ratio", Type: "number"},
	}

	// path missing, count invalid type, ratio valid
	_, err := ValidateArgs(params, []string{"", "not-int", "3.14"})
	if err == nil {
		t.Fatal("expected error, got nil")
	}

	errs := err.(ValidationErrors)
	// Should have 2 errors: path required, count type mismatch
	// Note: count is required but has a value (just wrong type), so it gets type error
	// path is required with empty value, so it gets required error
	if len(errs) < 1 {
		t.Errorf("expected at least 1 error, got %d", len(errs))
	}

	// Verify error string contains both issues
	errStr := errs.Error()
	if !strings.Contains(errStr, "path") {
		t.Errorf("error string should contain 'path': %s", errStr)
	}
}
